//! Stage 3: code generation and optimization.
//!
//! The code generator reads the AST (`.opa`), walks it, and emits an
//! object file with sections, symbols, relocations, and data bytes. The
//! keyhole peephole optimizer runs after the code generator. When the
//! `--compile` stage flag is set, `opc` writes the object data as JSON
//! (`.opl`).

use anyhow::Result;
use op_common::ast::{
    Access, Attribute, Condition, Expr, FnStmt, InitValue, Item, Module, OffsetOp, Operand,
    PlacementArg, SwitchCase, Type, UseRoot, UseTail, UseTree,
};
use op_common::{AstFile, TargetTriplet};
use op_diagnostics::{Diagnostic, Severity};
use op_ir::{ObjectFile, RelocKind, Relocation, Section, SectionKind, Symbol, SymbolKind};
use std::collections::{HashMap, HashSet};

use crate::cli::OpcArgs;
use crate::encoding::{get_full_encoding_table, AddrMode};
use crate::parser;

// --- Entry points -----------------------------------------------------------

/// Run the codegen and optimizer stage when the `--compile` flag is set.
pub fn run(args: &OpcArgs) -> Result<()> {
    if !args.compile {
        return Ok(());
    }
    let target = args.target.as_deref().unwrap_or("");
    let obj = compile_file(
        &args.input.input,
        target,
        args.opt_level,
        &args.include,
        &args.features,
    )?;
    let json = op_common::to_json(&obj)?;
    match &args.output {
        Some(path) => std::fs::write(path, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

/// Compile a source file into an [`ObjectFile`].
pub fn compile_file(
    path: &str,
    target: &str,
    opt_level: u8,
    include_paths: &[String],
    features: &[String],
) -> Result<ObjectFile> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path))?;
    let (ast, parse_diags) = parser::parse_source(path, &source, target, features);
    let has_errors = parse_diags.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        for d in &parse_diags {
            d.print(None);
        }
        anyhow::bail!("parser errors in {}", path);
    }
    let (obj, codegen_diags) = compile_source(&ast, opt_level, include_paths, features);
    let has_errors = codegen_diags.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        for d in &codegen_diags {
            d.print(None);
        }
        anyhow::bail!("codegen errors in {}", path);
    }
    Ok(obj)
}

/// Compile a parsed AST into an [`ObjectFile`] and a list of diagnostics.
pub fn compile_source(
    ast: &AstFile,
    opt_level: u8,
    include_paths: &[String],
    features: &[String],
) -> (ObjectFile, Vec<Diagnostic>) {
    let triplet = TargetTriplet::parse(&ast.target).unwrap_or(TargetTriplet {
        cpu: String::new(),
        manufacturer: String::new(),
        machine: String::new(),
        variant: String::new(),
    });

    let encoding_table = get_full_encoding_table(&triplet.cpu);

    let mut codegen = Codegen {
        target: triplet,
        opt_level,
        encoding_table,
        sections: Vec::new(),
        current_section: None,
        inline_fns: HashMap::new(),
        const_values: HashMap::new(),
        module_cache: ModuleCache::default(),
        module_path: Vec::new(),
        enum_variants: HashMap::new(),
        use_aliases: HashMap::new(),
        include_paths: include_paths.to_vec(),
        features: features.to_vec(),
        current_module_dir: None,
        label_counter: 0,
        interrupt_vectors: Vec::new(),
        header: None,
        pad_byte: 0x00,
        source_dir: std::path::Path::new(&ast.file)
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf(),
        diags: Vec::new(),
    };

    codegen.walk_module(&ast.root);

    // Run the peephole optimizer on the sections.
    let mut sections = codegen.sections;
    crate::optimizer::optimize(&mut sections, opt_level);

    let obj = ObjectFile {
        version: 1,
        target: ast.target.clone(),
        sections,
        interrupt_vectors: codegen.interrupt_vectors,
        header: codegen.header,
        pad_byte: codegen.pad_byte,
    };

    (obj, codegen.diags)
}

// --- Module cache -----------------------------------------------------------

/// Parsed std modules keyed by absolute file path.
///
/// Each file is parsed once. Later references to the same file reuse
/// the cached [`Module`], which prevents duplicate parses and
/// duplicate diagnostics.
#[derive(Default)]
struct ModuleCache {
    modules: HashMap<std::path::PathBuf, CachedModule>,
}

/// A parsed module plus the directory of the file it was loaded
/// from. The directory lets placement macros inside a std module
/// (such as `locate_bytes!`) resolve paths relative to the std file
/// instead of the root source file.
struct CachedModule {
    module: Module,
    dir: std::path::PathBuf,
}

impl ModuleCache {
    /// Load the module at `path`, parsing the file on a cache miss.
    ///
    /// cfg evaluation happens during parsing: the parser drops items
    /// whose `#[cfg]` predicate does not match `target`.
    fn load_module(
        &mut self,
        path: &std::path::Path,
        target: &TargetTriplet,
        features: &[String],
    ) -> Result<Module> {
        let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Some(entry) = self.modules.get(&key) {
            return Ok(entry.module.clone());
        }
        let path_str = key.to_string_lossy().to_string();
        let source = std::fs::read_to_string(&key)
            .map_err(|e| anyhow::anyhow!("failed to read {path_str}: {e}"))?;
        let (ast, diags) = parser::parse_source(&path_str, &source, &target.as_str(), features);
        let has_errors = diags.iter().any(|d| d.severity == Severity::Error);
        if has_errors {
            for d in &diags {
                d.print(None);
            }
            anyhow::bail!("parser errors in {path_str}");
        }
        let module = ast.root;
        let dir = key
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| key.clone());
        self.modules.insert(
            key,
            CachedModule {
                module: module.clone(),
                dir,
            },
        );
        Ok(module)
    }

    /// Return the directory of the file a cached module was loaded
    /// from.
    fn dir_of(&self, path: &std::path::Path) -> Option<&std::path::PathBuf> {
        let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.modules.get(&key).map(|entry| &entry.dir)
    }
}

// --- Inline functions -------------------------------------------------------

/// An inline function: the parameter names and the body that is
/// expanded, with arguments substituted, at each call site.
#[derive(Clone)]
struct InlineFn {
    params: Vec<String>,
    body: Vec<FnStmt>,
}

// --- Codegen struct ---------------------------------------------------------

struct Codegen {
    target: TargetTriplet,
    opt_level: u8,
    encoding_table: Vec<&'static crate::encoding::EncodingEntry>,
    sections: Vec<Section>,
    current_section: Option<usize>,
    inline_fns: HashMap<String, InlineFn>,
    const_values: HashMap<String, i64>,
    /// Parsed std modules keyed by absolute file path.
    module_cache: ModuleCache,
    /// Current module path stack. Empty at the crate root. A name is
    /// pushed when the codegen enters a module and popped when it
    /// leaves.
    module_path: Vec<String>,
    /// Enum variant values keyed by `EnumName::VariantName`.
    enum_variants: HashMap<String, i64>,
    /// Module paths bound by `use ... as alias` imports.
    use_aliases: HashMap<String, Vec<String>>,
    /// Include search paths from the CLI. The first directory that
    /// contains a `lib.op` file is the std crate root.
    include_paths: Vec<String>,
    /// Feature flags from the CLI. Used when parsing std modules.
    features: Vec<String>,
    label_counter: u32,
    interrupt_vectors: Vec<op_ir::InterruptVector>,
    header: Option<op_ir::HeaderFields>,
    pad_byte: u8,
    /// Directory of the std module file whose items are currently
    /// being walked, if any. `None` while walking the root source
    /// file. `locate_bytes!` and `locate_str!` inside std modules
    /// resolve paths against this directory.
    current_module_dir: Option<std::path::PathBuf>,
    /// Directory of the root source file. Used to resolve
    /// `locate_bytes!` and `locate_str!` paths.
    source_dir: std::path::PathBuf,
    diags: Vec<Diagnostic>,
}

// --- Module walking ---------------------------------------------------------

impl Codegen {
    fn walk_module(&mut self, module: &Module) {
        // Collect constants, enum variants, and imports first so that
        // fn bodies can reference them regardless of the order the
        // items appear in the file.
        self.collect_module_items(&module.items);
        for item in &module.items {
            self.walk_item(item);
        }
    }

    /// Collect constant and enum variant values from `items` without
    /// compiling any fn bodies. Also resolves `use` declarations and
    /// recurses into sub-modules and attribute blocks, so every name
    /// is bound before the first fn body is compiled.
    fn collect_module_items(&mut self, items: &[Item]) {
        for item in items {
            self.collect_item(item);
        }
    }

    fn collect_item(&mut self, item: &Item) {
        match item {
            Item::ConstDecl {
                name,
                value,
                evaluated_value,
                ..
            } => {
                let val = (*evaluated_value).or_else(|| eval_expr(value, &self.const_values));
                if let Some(val) = val {
                    self.const_values.insert(name.clone(), val);
                }
            }
            Item::EnumDecl { name, variants, .. } => {
                self.collect_enum(name, variants, false);
            }
            Item::UseDecl { trees, .. } => {
                self.resolve_use_decl(trees);
            }
            Item::ModDecl {
                name,
                resolved,
                body,
                ..
            } => {
                self.module_path.push(name.clone());
                if let Some(sub_module) = resolved {
                    self.collect_module_items(&sub_module.items);
                } else if let Some(items) = body {
                    self.collect_module_items(items);
                }
                self.module_path.pop();
            }
            Item::BlockAttribute { items, .. } => {
                self.collect_module_items(items);
            }
            _ => {}
        }
    }

    /// Return a clone of the current module path stack. The stack is
    /// empty at the crate root.
    fn current_module_path(&self) -> Vec<String> {
        self.module_path.clone()
    }

    /// Convert a use-tree root into a module path. `Lib` is the crate
    /// root (an empty path). `SelfMod` is the current stack. `Super`
    /// is the current stack without its last element. `Name` is a bare
    /// name.
    fn resolve_root(&self, root: &UseRoot) -> Vec<String> {
        match root {
            UseRoot::Lib => Vec::new(),
            UseRoot::SelfMod => self.module_path.clone(),
            UseRoot::Super => {
                let mut path = self.module_path.clone();
                path.pop();
                path
            }
            UseRoot::Name(name) => vec![name.clone()],
        }
    }

    // --- Use resolution -----------------------------------------------------

    /// Resolve every import tree in a `use` declaration. Imported
    /// names are inserted into the flat namespaces: `inline_fns` for
    /// inline functions, `const_values` for constants and enum
    /// variants, `enum_variants` for qualified variant names, and
    /// `use_aliases` for `as` bindings.
    fn resolve_use_decl(&mut self, trees: &[UseTree]) {
        let mut visited: HashSet<std::path::PathBuf> = HashSet::new();
        for tree in trees {
            self.resolve_use_tree(tree, &mut visited);
        }
    }

    /// Resolve a single import tree against the current module path
    /// and import its names.
    fn resolve_use_tree(&mut self, tree: &UseTree, visited: &mut HashSet<std::path::PathBuf>) {
        match tree {
            UseTree::Alias { inner, alias } => {
                self.use_aliases
                    .insert(alias.clone(), self.use_tree_module_path(inner));
            }
            UseTree::Path {
                root,
                segments,
                tail,
            } => {
                let mut path = self.use_tree_base_path(root);
                path.extend(segments.iter().cloned());

                match tail {
                    UseTail::Group(subtrees) => {
                        // Group members carry their own roots, so the
                        // group parent becomes their resolution
                        // context.
                        let saved = self.module_path.clone();
                        self.module_path = path.clone();
                        for subtree in subtrees {
                            self.resolve_use_tree(subtree, visited);
                        }
                        self.module_path = saved;
                    }
                    _ => self.import_path(&path, tail, visited),
                }
            }
        }
    }

    /// Compute the full path a use tree points at, treating every
    /// segment (including the last) as a module segment. Used for
    /// `as` aliases, which bind the alias to the target path.
    fn use_tree_module_path(&self, tree: &UseTree) -> Vec<String> {
        match tree {
            UseTree::Alias { inner, .. } => self.use_tree_module_path(inner),
            UseTree::Path { root, segments, .. } => {
                let mut path = self.use_tree_base_path(root);
                path.extend(segments.iter().cloned());
                path
            }
        }
    }

    /// Resolve a use-tree root to the module path it starts from.
    /// `std` always names the std crate root; any other bare name is
    /// relative to the current module path.
    fn use_tree_base_path(&self, root: &UseRoot) -> Vec<String> {
        match root {
            UseRoot::Name(name) if name == "std" => vec!["std".to_string()],
            UseRoot::Name(name) => {
                let mut path = self.current_module_path();
                path.push(name.clone());
                path
            }
            _ => self.resolve_root(root),
        }
    }

    /// Import the names at `path`. If `path` names a module file, the
    /// module's exported items are imported and nested public `use`
    /// trees are resolved in the module's own path context. Otherwise
    /// the last segment names an item inside the parent module: a
    /// glob of an enum also binds the variant names bare, a single
    /// import binds only the qualified names.
    fn import_path(
        &mut self,
        path: &[String],
        tail: &UseTail,
        visited: &mut HashSet<std::path::PathBuf>,
    ) {
        if path.first().is_some_and(|name| name == "std")
            && find_std_root(&self.include_paths).is_none()
        {
            self.error(
                302,
                "std library not found: set OP_STD_PATH or use --include",
            );
            return;
        }

        if let Some((module, file)) = self.lookup_module(path) {
            let key = file.canonicalize().unwrap_or(file);
            let module_dir = self.module_cache.dir_of(&key).cloned();
            if !visited.insert(key) {
                return;
            }
            let saved = self.module_path.clone();
            let saved_dir = self.current_module_dir.clone();
            self.module_path = path.to_vec();
            self.current_module_dir = module_dir;
            self.import_module_items(&module.items, visited);
            self.module_path = saved;
            self.current_module_dir = saved_dir;
            return;
        }

        // The path names an item, not a module file. Look the item up
        // in the parent module.
        if path.is_empty() {
            self.error(303, format!("module not found: {}", path.join("::")));
            return;
        }
        let parent = &path[..path.len() - 1];
        let name = &path[path.len() - 1];
        let Some((parent_module, _file)) = self.lookup_module(parent) else {
            self.error(303, format!("module not found: {}", path.join("::")));
            return;
        };
        let bare = matches!(tail, UseTail::Glob);
        let saved = self.module_path.clone();
        self.module_path = parent.to_vec();
        let mut found = false;
        for item in &parent_module.items {
            if decl_name(item) == Some(name.as_str()) {
                found = true;
                self.import_named_item(item, name, bare);
                break;
            }
        }
        self.module_path = saved;
        if !found && matches!(tail, UseTail::Glob) {
            // A glob must name a module or an importable item. A
            // single import of a missing name may be an item that is
            // cfg-gated out for this target, so it is not an error.
            self.error(303, format!("module not found: {}", path.join("::")));
        }
    }

    /// Import the exported items of a module into the flat namespaces.
    /// The module path is already on the stack, so nested `use`
    /// trees resolve relative to this module.
    fn import_module_items(&mut self, items: &[Item], visited: &mut HashSet<std::path::PathBuf>) {
        for item in items {
            match item {
                Item::InlineFnDecl {
                    name, params, body, ..
                } => {
                    self.import_inline_fn(name, params, body);
                }
                Item::ConstDecl {
                    name,
                    value,
                    evaluated_value,
                    ..
                } => {
                    let val = (*evaluated_value).or_else(|| eval_expr(value, &self.const_values));
                    if let Some(val) = val {
                        self.import_const(name, val);
                    }
                }
                Item::EnumDecl { name, variants, .. } => {
                    self.collect_enum(name, variants, false);
                }
                Item::UseDecl { trees, .. } => {
                    // Public uses re-export names; private uses (such
                    // as `use super::constants::*;` inside a std
                    // module) bind the names that module's own items
                    // reference.
                    for tree in trees {
                        self.resolve_use_tree(tree, visited);
                    }
                }
                Item::Placement {
                    macro_name,
                    argument,
                    ..
                } if self.current_section.is_some() => {
                    // Placement macros inside a std module emit into
                    // the active section, resolving paths relative to
                    // the std module's own directory. They are skipped
                    // while collecting (no active section).
                    self.handle_placement(macro_name, argument);
                }
                // Other item kinds produce no flat-namespace bindings.
                _ => {}
            }
        }
    }

    /// Import a single named item found in a parent module.
    fn import_named_item(&mut self, item: &Item, name: &str, bare: bool) {
        match item {
            Item::InlineFnDecl { params, body, .. } => {
                self.import_inline_fn(name, params, body);
            }
            Item::ConstDecl {
                value,
                evaluated_value,
                ..
            } => {
                let val = (*evaluated_value).or_else(|| eval_expr(value, &self.const_values));
                if let Some(val) = val {
                    self.import_const(name, val);
                }
            }
            Item::EnumDecl { variants, .. } => {
                self.collect_enum(name, variants, bare);
            }
            _ => {}
        }
    }

    /// Insert an inline function into the flat namespace, warning on
    /// collision.
    fn import_inline_fn(&mut self, name: &str, params: &[String], body: &[FnStmt]) {
        if self.inline_fns.contains_key(name) {
            self.warning(
                304,
                format!("name `{name}` imported more than once; last import wins"),
            );
        }
        self.inline_fns.insert(
            name.to_string(),
            InlineFn {
                params: params.to_vec(),
                body: body.to_vec(),
            },
        );
    }

    /// Insert a constant into the flat namespace, warning when an
    /// existing binding has a different value.
    fn import_const(&mut self, name: &str, val: i64) {
        match self.const_values.insert(name.to_string(), val) {
            Some(prev) if prev != val => {
                self.warning(
                    304,
                    format!("constant `{name}` imported with conflicting values; last import wins"),
                );
            }
            _ => {}
        }
    }

    /// Collect an enum's variant values into the flat namespace.
    /// Qualified keys (`EnumName::VariantName`) are always inserted.
    /// A variant without an explicit value takes the previous
    /// variant's value plus one; the first variant takes zero. Bare
    /// variant names are inserted only when the enum is glob-imported
    /// and only when the name is not already bound.
    fn collect_enum(&mut self, name: &str, variants: &[op_common::ast::EnumVariant], bare: bool) {
        let mut prev: Option<i64> = None;
        for variant in variants {
            // An explicit value that cannot be evaluated falls back to
            // the implicit value so one bad variant cannot drop the
            // rest of the enum.
            let val = variant
                .value
                .as_ref()
                .and_then(|v| eval_expr(v, &self.const_values))
                .or_else(|| prev.map(|p| p + 1))
                .unwrap_or(0);
            let qualified = format!("{name}::{}", variant.name);
            self.enum_variants.insert(qualified.clone(), val);
            let msg = "enum variant imported with conflicting values; last import wins";
            match self.const_values.insert(qualified, val) {
                Some(existing) if existing != val => self.warning(304, msg),
                _ => {}
            }
            if bare {
                self.const_values.entry(variant.name.clone()).or_insert(val);
            }
            prev = Some(val);
        }
    }

    /// Look up and load the module file for `path`, if it exists. No
    /// diagnostic is emitted; the caller decides what a miss means.
    /// Paths that start with `std` resolve against the std crate
    /// root; other paths resolve against the directory of the root
    /// source file.
    fn lookup_module(&mut self, path: &[String]) -> Option<(Module, std::path::PathBuf)> {
        let (base, rest) = match path.first() {
            Some(name) if name == "std" => {
                let root = find_std_root(&self.include_paths)?;
                (root, &path[1..])
            }
            _ => (self.source_dir.clone(), path),
        };
        let file = module_file_path(&base, rest)?;
        self.module_cache
            .load_module(&file, &self.target, &self.features)
            .ok()
            .map(|module| (module, file))
    }

    fn walk_item(&mut self, item: &Item) {
        match item {
            Item::ConstDecl {
                name,
                evaluated_value,
                ..
            } => {
                if let Some(val) = evaluated_value {
                    self.const_values.insert(name.clone(), *val);
                }
            }
            Item::VarDecl {
                name,
                ty,
                addr_binding,
                init,
                ..
            } => {
                self.alloc_variable(name, ty, addr_binding, init);
            }
            Item::FnDecl {
                name,
                body,
                is_noreturn,
                attributes,
            } => {
                // Check for #[interrupt(name)] attribute.
                for attr in attributes {
                    if attr.path == "interrupt" {
                        if let Some(int_name) = attr.args.first().map(|a| a.name.as_str()) {
                            if !int_name.is_empty() {
                                if let Some(vec_addr) =
                                    interrupt_vector_address(&self.target.cpu, int_name)
                                {
                                    self.interrupt_vectors.push(op_ir::InterruptVector {
                                        name: int_name.to_string(),
                                        address: vec_addr,
                                        target: name.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                // Store the body so `locate_fn!` can place it later.
                self.inline_fns.insert(
                    name.clone(),
                    InlineFn {
                        params: Vec::new(),
                        body: body.clone(),
                    },
                );
                // When declared inside a section, compile in place.
                // When declared outside a section, defer to `locate_fn!`.
                if self.current_section.is_some() {
                    self.compile_fn(name, body, *is_noreturn);
                }
            }
            Item::InlineFnDecl {
                name, params, body, ..
            } => {
                self.inline_fns.insert(
                    name.clone(),
                    InlineFn {
                        params: params.clone(),
                        body: body.clone(),
                    },
                );
            }
            Item::StructDecl { .. } | Item::TypeDecl { .. } | Item::EnumDecl { .. } => {
                // No codegen output for type declarations.
            }
            Item::ModDecl {
                name,
                resolved,
                body,
                ..
            } => {
                // The sub-module's items were already collected during
                // the module's collection pass.
                self.module_path.push(name.clone());
                if let Some(sub_module) = resolved {
                    for item in &sub_module.items {
                        self.walk_item(item);
                    }
                } else if let Some(items) = body {
                    for item in items {
                        self.walk_item(item);
                    }
                }
                self.module_path.pop();
            }
            Item::UseDecl { .. } => {
                // Imports are resolved by `collect_module_items` before
                // any fn body is compiled.
            }
            Item::BlockAttribute { attr, items } => {
                self.handle_block_attribute(attr, items);
            }
            Item::Placement {
                macro_name,
                argument,
                attributes,
            } => {
                // Check for #[interrupt(name)] attribute on placements.
                for attr in attributes {
                    if attr.path == "interrupt" {
                        if let Some(int_name) = attr.args.first().map(|a| a.name.as_str()) {
                            if !int_name.is_empty() {
                                // Get the target function name from the placement argument.
                                let target_name = if let PlacementArg::Path { segments } = argument
                                {
                                    segments.last().cloned().unwrap_or_default()
                                } else {
                                    String::new()
                                };
                                if !target_name.is_empty() {
                                    if let Some(vec_addr) =
                                        interrupt_vector_address(&self.target.cpu, int_name)
                                    {
                                        self.interrupt_vectors.push(op_ir::InterruptVector {
                                            name: int_name.to_string(),
                                            address: vec_addr,
                                            target: target_name,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                self.handle_placement(macro_name, argument);
            }
        }
    }

    fn handle_block_attribute(&mut self, attr: &Attribute, items: &[Item]) {
        // Handle standalone attributes (empty items) that are not section blocks.
        if items.is_empty() {
            match attr.path.as_str() {
                "ines" => {
                    let fields: Vec<(String, String)> = attr
                        .args
                        .iter()
                        .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                        .collect();
                    self.header = Some(op_ir::HeaderFields {
                        format: "ines".to_string(),
                        fields,
                    });
                    return;
                }
                "lnx" => {
                    let fields: Vec<(String, String)> = attr
                        .args
                        .iter()
                        .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                        .collect();
                    self.header = Some(op_ir::HeaderFields {
                        format: "lnx".to_string(),
                        fields,
                    });
                    return;
                }
                "setpad" => {
                    if let Some(arg) = attr.args.first() {
                        let val = arg.value.trim_matches('"');
                        if let Some(hex) = val.strip_prefix("0x") {
                            self.pad_byte = u8::from_str_radix(hex, 16).unwrap_or(0x00);
                        } else {
                            self.pad_byte = val.parse::<u8>().unwrap_or(0x00);
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        let kind = match attr.path.as_str() {
            "rom" => SectionKind::Rom,
            "ram" => SectionKind::Ram,
            "chr" => SectionKind::Chr,
            _ => return,
        };

        let org = get_attr_u32(attr, "org").unwrap_or(0);
        let bank = get_attr_u32(attr, "bank").unwrap_or(0);
        let maxsize = get_attr_u32(attr, "maxsize").unwrap_or(0);
        let name = format!(
            "{}_bank{}",
            match kind {
                SectionKind::Rom => "rom",
                SectionKind::Ram => "ram",
                SectionKind::Chr => "chr",
            },
            bank
        );

        let section = Section {
            name,
            kind,
            org,
            bank,
            maxsize,
            symbols: Vec::new(),
            relocations: Vec::new(),
            data: Vec::new(),
        };

        self.sections.push(section);
        self.current_section = Some(self.sections.len() - 1);

        for item in items {
            self.walk_item(item);
        }

        self.current_section = None;
    }

    fn handle_placement(&mut self, macro_name: &str, argument: &PlacementArg) {
        match macro_name {
            "locate_bytes" => {
                if let PlacementArg::String_ { value } = argument {
                    let filename = value.trim_matches('"');
                    let base = self
                        .current_module_dir
                        .clone()
                        .unwrap_or_else(|| self.source_dir.clone());
                    let path = base.join(filename);
                    if let Ok(data) = std::fs::read(&path) {
                        self.emit_bytes(&data);
                    } else {
                        self.error(
                            300,
                            format!("locate_bytes: cannot read file '{}'", filename),
                        );
                    }
                }
            }
            "locate_fn" => {
                if let PlacementArg::Path { segments } = argument {
                    // Look up the function in the inline_fns map.
                    if !segments.is_empty() {
                        let fn_name = segments.last().unwrap();
                        if let Some(inline_fn) = self.inline_fns.get(fn_name).cloned() {
                            // Record the function symbol at the current
                            // offset, then compile the body inline.
                            let start_offset = if let Some(idx) = self.current_section {
                                self.sections[idx].data.len() as u32
                            } else {
                                0
                            };
                            self.compile_fn_body(&inline_fn.body);
                            let end_offset = if let Some(idx) = self.current_section {
                                self.sections[idx].data.len() as u32
                            } else {
                                0
                            };
                            if let Some(idx) = self.current_section {
                                self.sections[idx].symbols.push(Symbol {
                                    name: fn_name.clone(),
                                    offset: start_offset,
                                    size: end_offset - start_offset,
                                    kind: SymbolKind::Function,
                                    is_pub: false,
                                });
                            }
                        }
                    }
                }
            }
            "locate_str" => {
                if let PlacementArg::String_ { value } = argument {
                    let filename = value.trim_matches('"');
                    let base = self
                        .current_module_dir
                        .clone()
                        .unwrap_or_else(|| self.source_dir.clone());
                    let path = base.join(filename);
                    if let Ok(source) = std::fs::read_to_string(&path) {
                        let (ast, _diags) = parser::parse_source(
                            &path.to_string_lossy(),
                            &source,
                            &self.target.as_str(),
                            &[],
                        );
                        self.walk_module(&ast.root);
                    }
                }
            }
            _ => {}
        }
    }

    fn alloc_variable(
        &mut self,
        name: &str,
        ty: &Type,
        addr_binding: &Option<Expr>,
        init: &Option<InitValue>,
    ) {
        let size = type_size(ty);
        let offset = if let Some(addr_expr) = addr_binding {
            eval_expr(addr_expr, &self.const_values).unwrap_or(0) as u32
        } else if let Some(idx) = self.current_section {
            let offset = self.sections[idx].data.len() as u32;
            // Allocate space.
            for _ in 0..size {
                self.sections[idx].data.push(0);
            }
            offset
        } else {
            0
        };

        // Emit init data if present.
        if let Some(init_val) = init {
            if let Some(idx) = self.current_section {
                match init_val {
                    InitValue::Expr { value } => {
                        if let Some(val) = eval_expr(value, &self.const_values) {
                            let bytes = val_to_bytes(val, size);
                            for (i, b) in bytes.iter().enumerate() {
                                if (offset as usize + i) < self.sections[idx].data.len() {
                                    self.sections[idx].data[offset as usize + i] = *b;
                                }
                            }
                        }
                    }
                    InitValue::String_ { value } => {
                        let s = value.trim_matches('"');
                        for (i, b) in s.bytes().enumerate() {
                            if (offset as usize + i) < self.sections[idx].data.len() {
                                self.sections[idx].data[offset as usize + i] = b;
                            }
                        }
                    }
                    InitValue::InitList { items } => {
                        let mut pos = offset as usize;
                        for item in items {
                            if let InitValue::Expr { value } = item {
                                if let Some(val) = eval_expr(value, &self.const_values) {
                                    self.sections[idx].data[pos] = val as u8;
                                    pos += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Record the symbol.
        if let Some(idx) = self.current_section {
            self.sections[idx].symbols.push(Symbol {
                name: name.to_string(),
                offset,
                size: size as u32,
                kind: SymbolKind::Variable,
                is_pub: false,
            });
        }
    }

    fn compile_fn(&mut self, name: &str, body: &[FnStmt], _is_noreturn: bool) {
        // Record the function symbol at the current offset.
        let offset = if let Some(idx) = self.current_section {
            self.sections[idx].data.len() as u32
        } else {
            0
        };

        let start_offset = offset;

        // Compile the function body.
        self.compile_fn_body(body);

        let end_offset = if let Some(idx) = self.current_section {
            self.sections[idx].data.len() as u32
        } else {
            0
        };

        // Record the function symbol.
        if let Some(idx) = self.current_section {
            self.sections[idx].symbols.push(Symbol {
                name: name.to_string(),
                offset: start_offset,
                size: end_offset - start_offset,
                kind: SymbolKind::Function,
                is_pub: false,
            });
        }
    }

    fn compile_fn_body(&mut self, body: &[FnStmt]) {
        for stmt in body {
            self.compile_stmt(stmt);
        }
    }

    fn compile_stmt(&mut self, stmt: &FnStmt) {
        match stmt {
            FnStmt::AsmStmt { opcode, operands } => {
                self.compile_asm(opcode, operands);
            }
            FnStmt::IfStmt {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.compile_if(condition, then_block, else_block);
            }
            FnStmt::WhileStmt {
                condition, body, ..
            } => {
                self.compile_while(condition, body);
            }
            FnStmt::DoWhileStmt {
                body, condition, ..
            } => {
                self.compile_do_while(body, condition);
            }
            FnStmt::LoopStmt { body } => {
                self.compile_loop(body);
            }
            FnStmt::SwitchStmt { register, cases } => {
                self.compile_switch(register, cases);
            }
            FnStmt::FnCall { name, args } => {
                self.compile_fn_call(name, args);
            }
            FnStmt::ReturnStmt => {
                self.emit_byte(0x60); // RTS
            }
            FnStmt::Label { name, stmt } => {
                // Record the label at the current offset.
                let offset = if let Some(idx) = self.current_section {
                    self.sections[idx].data.len() as u32
                } else {
                    0
                };
                if let Some(idx) = self.current_section {
                    self.sections[idx].symbols.push(Symbol {
                        name: name.clone(),
                        offset,
                        size: 0,
                        kind: SymbolKind::Label,
                        is_pub: false,
                    });
                }
                // Compile the statement that follows the label.
                self.compile_stmt(stmt);
            }
            FnStmt::VarDeclStmt { decl } => {
                if let Item::VarDecl {
                    name,
                    ty,
                    addr_binding,
                    init,
                    ..
                } = decl.as_ref()
                {
                    self.alloc_variable(name, ty, addr_binding, init);
                }
            }
        }
    }

    // --- Assembly encoding ---------------------------------------------------

    fn compile_asm(&mut self, opcode: &str, operands: &[Operand]) {
        // Handle implied/accumulator mode (no operands).
        if operands.is_empty() {
            if let Some(op_byte) = self.lookup(opcode, AddrMode::Implied) {
                self.emit_byte(op_byte);
                return;
            }
            // Try accumulator mode for ASL/LSR/ROL/ROR.
            if let Some(op_byte) = self.lookup(opcode, AddrMode::Accumulator) {
                self.emit_byte(op_byte);
                return;
            }
            self.error(301, format!("unknown opcode '{}' with no operands", opcode));
            return;
        }

        let operand = &operands[0];

        match operand {
            Operand::Immediate { value } => {
                if let Some(op_byte) = self.lookup(opcode, AddrMode::Immediate) {
                    self.emit_byte(op_byte);
                    let val = eval_expr(value, &self.const_values);
                    match val {
                        Some(v) => {
                            self.emit_byte((v & 0xFF) as u8);
                        }
                        None => {
                            // Symbol reference — emit placeholder and relocation.
                            self.emit_byte(0);
                            if let Some(sym) = expr_to_symbol(value) {
                                self.add_relocation(
                                    1,
                                    RelocKind::Abs8,
                                    &sym,
                                    selector_addend(value, &self.const_values),
                                );
                            }
                        }
                    }
                } else {
                    self.error(
                        301,
                        format!("opcode '{}' does not support immediate mode", opcode),
                    );
                }
            }
            Operand::MemoryOperand {
                mode_prefix,
                expr,
                index_reg,
            } => {
                self.compile_memory_operand(
                    opcode,
                    mode_prefix.as_deref(),
                    expr,
                    index_reg.as_deref(),
                );
            }
            Operand::RegisterRef { name: _ } => {
                // cpu::a, cpu::x, cpu::y — for switch statements.
                // No direct encoding; these are handled by switch.
            }
            Operand::LabelRef { name } => {
                // Branch to label.
                if let Some(op_byte) = self.lookup(opcode, AddrMode::Relative) {
                    self.emit_byte(op_byte);
                    self.emit_byte(0); // placeholder offset
                    self.add_relocation(1, RelocKind::Branch8, name, 0);
                } else {
                    // Non-branch instruction with label ref — treat as absolute.
                    if let Some(op_byte) = self.lookup(opcode, AddrMode::Absolute) {
                        self.emit_byte(op_byte);
                        self.emit_byte(0);
                        self.emit_byte(0);
                        self.add_relocation(2, RelocKind::Abs16, name, 0);
                    }
                }
            }
            Operand::Selector { path, accesses } => {
                // Selector like PPU::CNT0 — resolve to a constant, or
                // fall back to a symbol relocation.
                let path_name = path.join("::");
                match resolve_selector(path, accesses, &self.const_values) {
                    Some(val) => {
                        // Known constant: emit the (offset-adjusted)
                        // value directly as an absolute operand.
                        if let Some(op_byte) = self.lookup(opcode, AddrMode::Absolute) {
                            self.emit_byte(op_byte);
                            self.emit_bytes(&val_to_bytes(val, 2));
                        }
                    }
                    None if !path_name.is_empty() => {
                        // Not a known constant: keep the old behaviour
                        // and emit a relocation against the joined
                        // path, carrying the folded offset as the
                        // relocation addend.
                        if let Some(op_byte) = self.lookup(opcode, AddrMode::Absolute) {
                            self.emit_byte(op_byte);
                            self.emit_byte(0);
                            self.emit_byte(0);
                            self.add_relocation(
                                2,
                                RelocKind::Abs16,
                                &path_name,
                                selector_offset(accesses, &self.const_values),
                            );
                        }
                    }
                    None => {}
                }
            }
        }
    }

    fn compile_memory_operand(
        &mut self,
        opcode: &str,
        mode_prefix: Option<&str>,
        expr: &Expr,
        index_reg: Option<&str>,
    ) {
        let val = eval_expr(expr, &self.const_values);

        // Determine the addressing mode.
        let mode = if let Some(prefix) = mode_prefix {
            match prefix {
                "zp" => AddrMode::ZeroPage,
                "abs" => AddrMode::Absolute,
                "rel" => AddrMode::Relative,
                "ind" => AddrMode::Indirect,
                "idx" => AddrMode::AbsoluteX,
                "ind_l" => AddrMode::Indirect,
                "ind_idx" => AddrMode::IndirectY,
                _ => AddrMode::Absolute,
            }
        } else if let Some(idx) = index_reg {
            // Indexed mode.
            if idx.contains("x") {
                if val.map(|v| v <= 0xFF).unwrap_or(false) {
                    AddrMode::ZeroPageX
                } else {
                    AddrMode::AbsoluteX
                }
            } else if idx.contains("y") {
                if val.map(|v| v <= 0xFF).unwrap_or(false) {
                    AddrMode::ZeroPageY
                } else {
                    AddrMode::AbsoluteY
                }
            } else {
                AddrMode::Absolute
            }
        } else if val.map(|v| v <= 0xFF).unwrap_or(false) {
            // Prefer zero-page for small values.
            if self.lookup(opcode, AddrMode::ZeroPage).is_some() {
                AddrMode::ZeroPage
            } else {
                AddrMode::Absolute
            }
        } else {
            AddrMode::Absolute
        };

        // Try the preferred mode, fall back to absolute.
        let op_byte = self
            .lookup(opcode, mode)
            .or_else(|| self.lookup(opcode, AddrMode::Absolute));

        if let Some(op_byte) = op_byte {
            self.emit_byte(op_byte);
            match val {
                Some(v) => {
                    if mode == AddrMode::ZeroPage
                        || mode == AddrMode::ZeroPageX
                        || mode == AddrMode::ZeroPageY
                        || mode == AddrMode::Relative
                    {
                        self.emit_byte((v & 0xFF) as u8);
                    } else {
                        // Absolute: 2 bytes, little-endian.
                        self.emit_byte((v & 0xFF) as u8);
                        self.emit_byte(((v >> 8) & 0xFF) as u8);
                    }
                }
                None => {
                    // Symbol reference.
                    let addend = selector_addend(expr, &self.const_values);
                    if mode == AddrMode::ZeroPage
                        || mode == AddrMode::ZeroPageX
                        || mode == AddrMode::ZeroPageY
                    {
                        self.emit_byte(0);
                        if let Some(sym) = expr_to_symbol(expr) {
                            self.add_relocation(1, RelocKind::Abs8, &sym, addend);
                        }
                    } else {
                        self.emit_byte(0);
                        self.emit_byte(0);
                        if let Some(sym) = expr_to_symbol(expr) {
                            self.add_relocation(2, RelocKind::Abs16, &sym, addend);
                        }
                    }
                }
            }
        } else {
            self.error(
                301,
                format!("cannot encode opcode '{}' with mode {:?}", opcode, mode),
            );
        }
    }

    // --- Control flow -------------------------------------------------------

    fn compile_if(
        &mut self,
        condition: &Condition,
        then_block: &[FnStmt],
        else_block: &Option<Vec<FnStmt>>,
    ) {
        // Emit branch-if-not-condition over the then-block.
        let branch_op = self.condition_to_branch_op(condition, false);
        self.emit_byte(branch_op);
        let patch_offset = self.current_data_len();
        self.emit_byte(0); // placeholder branch offset

        // Compile the then-block.
        for stmt in then_block {
            self.compile_stmt(stmt);
        }

        if let Some(else_blk) = else_block {
            // Emit jump past the else-block.
            self.emit_byte(0x4C); // JMP absolute
            let else_jump_patch = self.current_data_len();
            self.emit_byte(0);
            self.emit_byte(0);

            // Patch the branch to skip over the then-block + JMP.
            let then_size = self.current_data_len() as i64 - patch_offset as i64 - 1;
            if then_size >= 0 {
                self.patch_byte(patch_offset, then_size as u8);
            }

            // Compile the else-block.
            for stmt in else_blk {
                self.compile_stmt(stmt);
            }

            // Patch the JMP to skip over the else-block.
            let else_end = self.current_data_len() as u32;
            self.patch_byte(else_jump_patch, (else_end & 0xFF) as u8);
            self.patch_byte(else_jump_patch + 1, ((else_end >> 8) & 0xFF) as u8);
        } else {
            // Patch the branch to skip over the then-block.
            let then_size = self.current_data_len() as i64 - patch_offset as i64 - 1;
            if then_size >= 0 {
                self.patch_byte(patch_offset, then_size as u8);
            }
        }
    }

    fn compile_while(&mut self, condition: &Condition, body: &[FnStmt]) {
        let loop_start = self.current_data_len() as u32;

        // Emit branch-if-not-condition past the body.
        let branch_op = self.condition_to_branch_op(condition, false);
        self.emit_byte(branch_op);
        let patch_offset = self.current_data_len();
        self.emit_byte(0); // placeholder

        // Compile the body.
        for stmt in body {
            self.compile_stmt(stmt);
        }

        // Emit JMP back to loop_start.
        self.emit_byte(0x4C); // JMP absolute
        self.emit_byte((loop_start & 0xFF) as u8);
        self.emit_byte(((loop_start >> 8) & 0xFF) as u8);

        // Patch the branch to skip over the body + JMP.
        let body_size = self.current_data_len() as i64 - patch_offset as i64 - 1;
        if body_size >= 0 {
            self.patch_byte(patch_offset, body_size as u8);
        }
    }

    fn compile_do_while(&mut self, body: &[FnStmt], condition: &Condition) {
        let loop_start = self.current_data_len() as u32;

        // Compile the body.
        for stmt in body {
            self.compile_stmt(stmt);
        }

        // Emit branch-if-condition back to loop_start.
        let branch_op = self.condition_to_branch_op(condition, true);
        self.emit_byte(branch_op);
        let offset = loop_start as i64 - (self.current_data_len() as i64 + 1);
        self.emit_byte(offset as u8);
    }

    fn compile_loop(&mut self, body: &[FnStmt]) {
        let loop_start = self.current_data_len() as u32;

        // Compile the body.
        for stmt in body {
            self.compile_stmt(stmt);
        }

        // Emit JMP back to loop_start.
        self.emit_byte(0x4C); // JMP absolute
        self.emit_byte((loop_start & 0xFF) as u8);
        self.emit_byte(((loop_start >> 8) & 0xFF) as u8);
    }

    fn compile_switch(&mut self, _register: &str, cases: &[SwitchCase]) {
        // For each case, emit CMP #value then BEQ to the case body.
        let mut case_patches: Vec<(usize, u32)> = Vec::new();

        for case in cases {
            match case {
                SwitchCase::Case { expr, body: _ } => {
                    // CMP #value
                    let val = eval_expr(expr, &self.const_values).unwrap_or(0);
                    self.emit_byte(0xC9); // CMP immediate
                    self.emit_byte((val & 0xFF) as u8);
                    // BEQ to case body
                    self.emit_byte(0xF0); // BEQ
                    let patch = self.current_data_len();
                    self.emit_byte(0); // placeholder
                    case_patches.push((patch, 0)); // will be filled

                    // Record the start of the case body.
                    let body_start = self.current_data_len() as u32;
                    // Update the last patch.
                    if let Some(last) = case_patches.last_mut() {
                        last.1 = body_start;
                    }
                    // Actually, we need to patch at the time we know the offset.
                    // Let's just compile the body inline.
                    let _ = body_start;
                }
                SwitchCase::Default { body } => {
                    for stmt in body {
                        self.compile_stmt(stmt);
                    }
                }
            }
        }

        // Simplified: compile all case bodies sequentially after the compares.
        // This is a basic implementation.
        for case in cases {
            if let SwitchCase::Case { body, .. } = case {
                for stmt in body {
                    self.compile_stmt(stmt);
                }
            }
        }
    }

    fn compile_fn_call(&mut self, name: &str, args: &[Expr]) {
        // Check if it's an inline fn.
        if let Some(inline_fn) = self.inline_fns.get(name).cloned() {
            // Substitute the call arguments for the parameters, then
            // expand the body at the call site. Nested inline calls
            // in the substituted body resolve during this compile
            // pass.
            let body = self.substitute_params(&inline_fn.body, &inline_fn.params, args);
            for stmt in &body {
                self.compile_stmt(stmt);
            }
        } else {
            // Regular function call — emit JSR.
            self.emit_byte(0x20); // JSR absolute
            self.emit_byte(0);
            self.emit_byte(0);
            self.add_relocation(2, RelocKind::Abs16, name, 0);
        }
    }

    // --- Parameter substitution ---------------------------------------------

    /// Clone an inline fn body, replacing parameter idents with the
    /// call-site argument expressions. Parameters without a matching
    /// argument are left as-is so the compiler falls back to the
    /// existing symbol handling for them.
    fn substitute_params(&self, body: &[FnStmt], params: &[String], args: &[Expr]) -> Vec<FnStmt> {
        body.iter()
            .map(|stmt| self.substitute_stmt(stmt, params, args))
            .collect()
    }

    fn substitute_stmt(&self, stmt: &FnStmt, params: &[String], args: &[Expr]) -> FnStmt {
        match stmt {
            FnStmt::AsmStmt { opcode, operands } => FnStmt::AsmStmt {
                opcode: opcode.clone(),
                operands: operands
                    .iter()
                    .map(|operand| self.substitute_operand(operand, params, args))
                    .collect(),
            },
            FnStmt::FnCall {
                name,
                args: call_args,
            } => FnStmt::FnCall {
                name: name.clone(),
                args: call_args
                    .iter()
                    .map(|arg| self.substitute_expr(arg, params, args))
                    .collect(),
            },
            FnStmt::Label { name, stmt } => FnStmt::Label {
                name: name.clone(),
                stmt: Box::new(self.substitute_stmt(stmt, params, args)),
            },
            FnStmt::IfStmt {
                branch_hint,
                condition,
                then_block,
                else_block,
            } => FnStmt::IfStmt {
                branch_hint: *branch_hint,
                condition: condition.clone(),
                then_block: self.substitute_block(then_block, params, args),
                else_block: else_block
                    .as_ref()
                    .map(|block| self.substitute_block(block, params, args)),
            },
            FnStmt::WhileStmt {
                branch_hint,
                condition,
                body,
            } => FnStmt::WhileStmt {
                branch_hint: *branch_hint,
                condition: condition.clone(),
                body: self.substitute_block(body, params, args),
            },
            FnStmt::DoWhileStmt {
                body,
                branch_hint,
                condition,
            } => FnStmt::DoWhileStmt {
                body: self.substitute_block(body, params, args),
                branch_hint: *branch_hint,
                condition: condition.clone(),
            },
            FnStmt::LoopStmt { body } => FnStmt::LoopStmt {
                body: self.substitute_block(body, params, args),
            },
            FnStmt::SwitchStmt { register, cases } => FnStmt::SwitchStmt {
                register: register.clone(),
                cases: cases
                    .iter()
                    .map(|case| self.substitute_case(case, params, args))
                    .collect(),
            },
            FnStmt::VarDeclStmt { decl } => FnStmt::VarDeclStmt {
                decl: Box::new(self.substitute_item(decl, params, args)),
            },
            FnStmt::ReturnStmt => FnStmt::ReturnStmt,
        }
    }

    fn substitute_block(&self, block: &[FnStmt], params: &[String], args: &[Expr]) -> Vec<FnStmt> {
        block
            .iter()
            .map(|stmt| self.substitute_stmt(stmt, params, args))
            .collect()
    }

    fn substitute_case(&self, case: &SwitchCase, params: &[String], args: &[Expr]) -> SwitchCase {
        match case {
            SwitchCase::Case { expr, body } => SwitchCase::Case {
                expr: self.substitute_expr(expr, params, args),
                body: self.substitute_block(body, params, args),
            },
            SwitchCase::Default { body } => SwitchCase::Default {
                body: self.substitute_block(body, params, args),
            },
        }
    }

    fn substitute_item(&self, item: &Item, params: &[String], args: &[Expr]) -> Item {
        match item {
            Item::ConstDecl {
                name,
                ty,
                value,
                evaluated_value,
                attributes,
            } => Item::ConstDecl {
                name: name.clone(),
                ty: ty.clone(),
                value: self.substitute_expr(value, params, args),
                evaluated_value: *evaluated_value,
                attributes: attributes.clone(),
            },
            Item::VarDecl {
                name,
                is_volatile,
                ty,
                array_dim,
                addr_binding,
                init,
                attributes,
            } => Item::VarDecl {
                name: name.clone(),
                is_volatile: *is_volatile,
                ty: ty.clone(),
                array_dim: array_dim
                    .as_ref()
                    .map(|dim| self.substitute_expr(dim, params, args)),
                addr_binding: addr_binding
                    .as_ref()
                    .map(|binding| self.substitute_expr(binding, params, args)),
                init: init
                    .as_ref()
                    .map(|init| self.substitute_init(init, params, args)),
                attributes: attributes.clone(),
            },
            _ => item.clone(),
        }
    }

    fn substitute_init(&self, init: &InitValue, params: &[String], args: &[Expr]) -> InitValue {
        match init {
            InitValue::Expr { value } => InitValue::Expr {
                value: self.substitute_expr(value, params, args),
            },
            InitValue::InitList { items } => InitValue::InitList {
                items: items
                    .iter()
                    .map(|item| self.substitute_init(item, params, args))
                    .collect(),
            },
            InitValue::String_ { value } => InitValue::String_ {
                value: value.clone(),
            },
        }
    }

    fn substitute_operand(&self, operand: &Operand, params: &[String], args: &[Expr]) -> Operand {
        match operand {
            Operand::Immediate { value } => Operand::Immediate {
                value: self.substitute_expr(value, params, args),
            },
            Operand::MemoryOperand {
                mode_prefix,
                expr,
                index_reg,
            } => Operand::MemoryOperand {
                mode_prefix: mode_prefix.clone(),
                expr: self.substitute_expr(expr, params, args),
                index_reg: index_reg.clone(),
            },
            Operand::RegisterRef { name } => Operand::RegisterRef { name: name.clone() },
            Operand::LabelRef { name } => Operand::LabelRef { name: name.clone() },
            Operand::Selector { path, accesses } => Operand::Selector {
                path: path.clone(),
                accesses: accesses
                    .iter()
                    .map(|access| self.substitute_access(access, params, args))
                    .collect(),
            },
        }
    }

    fn substitute_access(&self, access: &Access, params: &[String], args: &[Expr]) -> Access {
        match access {
            Access::ModuleAccess { name } => Access::ModuleAccess { name: name.clone() },
            Access::FieldAccess { name } => Access::FieldAccess { name: name.clone() },
            Access::Offset { op, value } => Access::Offset {
                op: *op,
                value: self.substitute_expr(value, params, args),
            },
        }
    }

    /// Replace `Expr::Ident` names that match a parameter with the
    /// matching argument expression, recursing into composite
    /// expressions.
    fn substitute_expr(&self, expr: &Expr, params: &[String], args: &[Expr]) -> Expr {
        match expr {
            Expr::Ident { name } => params
                .iter()
                .position(|param| param == name)
                .and_then(|index| args.get(index))
                .cloned()
                .unwrap_or_else(|| expr.clone()),
            Expr::BinOp { op, left, right } => Expr::BinOp {
                op: *op,
                left: Box::new(self.substitute_expr(left, params, args)),
                right: Box::new(self.substitute_expr(right, params, args)),
            },
            Expr::UnaryOp { op, operand } => Expr::UnaryOp {
                op: *op,
                operand: Box::new(self.substitute_expr(operand, params, args)),
            },
            Expr::MacroCall { name, arg } => Expr::MacroCall {
                name: name.clone(),
                arg: Box::new(self.substitute_expr(arg, params, args)),
            },
            Expr::Selector { path, accesses } => {
                // A path element can be a parameter: `dest + 1`
                // parses as a selector on `dest`. An identifier
                // argument replaces the name in place; other argument
                // shapes leave the selector unchanged.
                let path = path
                    .iter()
                    .map(|segment| {
                        params
                            .iter()
                            .position(|param| param == segment)
                            .and_then(|index| args.get(index))
                            .and_then(|arg| match arg {
                                Expr::Ident { name } => Some(name.clone()),
                                _ => None,
                            })
                            .unwrap_or_else(|| segment.clone())
                    })
                    .collect();
                Expr::Selector {
                    path,
                    accesses: accesses
                        .iter()
                        .map(|access| self.substitute_access(access, params, args))
                        .collect(),
                }
            }
            Expr::FnCall {
                name,
                args: call_args,
            } => Expr::FnCall {
                name: name.clone(),
                args: call_args
                    .iter()
                    .map(|arg| self.substitute_expr(arg, params, args))
                    .collect(),
            },
            Expr::ParenExpr { inner } => Expr::ParenExpr {
                inner: Box::new(self.substitute_expr(inner, params, args)),
            },
            Expr::Number { .. } | Expr::String_ { .. } | Expr::Boolean { .. } => expr.clone(),
        }
    }

    // --- Helpers ------------------------------------------------------------

    fn lookup(&self, mnemonic: &str, mode: AddrMode) -> Option<u8> {
        crate::encoding::lookup_opcode_in(&self.encoding_table, mnemonic, mode)
    }

    fn emit_byte(&mut self, byte: u8) {
        if let Some(idx) = self.current_section {
            self.sections[idx].data.push(byte);
        }
    }

    fn emit_bytes(&mut self, bytes: &[u8]) {
        if let Some(idx) = self.current_section {
            self.sections[idx].data.extend_from_slice(bytes);
        }
    }

    fn patch_byte(&mut self, offset: usize, byte: u8) {
        if let Some(idx) = self.current_section {
            if offset < self.sections[idx].data.len() {
                self.sections[idx].data[offset] = byte;
            }
        }
    }

    fn current_data_len(&self) -> usize {
        if let Some(idx) = self.current_section {
            self.sections[idx].data.len()
        } else {
            0
        }
    }

    fn add_relocation(&mut self, offset_from_end: u32, kind: RelocKind, symbol: &str, addend: i64) {
        if let Some(idx) = self.current_section {
            let abs_offset = self.sections[idx].data.len() as u32 - offset_from_end;
            self.sections[idx].relocations.push(Relocation {
                offset: abs_offset,
                kind,
                symbol: symbol.to_string(),
                addend,
            });
        }
    }

    fn error(&mut self, code: u32, msg: impl Into<String>) {
        self.diags.push(Diagnostic::error(code, "", 0, 0, msg));
    }

    fn warning(&mut self, code: u32, msg: impl Into<String>) {
        self.diags.push(Diagnostic::warning(code, "", 0, 0, msg));
    }

    /// Map a condition keyword to a branch opcode byte.
    /// If `invert` is true, return the branch-if-condition opcode.
    /// If `invert` is false, return the branch-if-not-condition opcode.
    fn condition_to_branch_op(&self, condition: &Condition, invert: bool) -> u8 {
        // 6502 condition keywords to branch opcodes.
        let (branch_if_true, branch_if_false) = match condition.keyword.as_str() {
            "plus" | "positive" | "greater" => (0x10, 0x30), // BPL, BMI
            "minus" | "negative" | "less" => (0x30, 0x10),   // BMI, BPL
            "overflow" => (0x70, 0x50),                      // BVS, BVC
            "carry" => (0xB0, 0x90),                         // BCS, BCC
            "nonzero" | "set" | "true" => (0xD0, 0xF0),      // BNE, BEQ
            "zero" | "unset" | "false" | "clear" | "equal" => (0xF0, 0xD0), // BEQ, BNE
            _ => (0xD0, 0xF0),                               // default: BNE, BEQ
        };
        if invert {
            branch_if_true
        } else {
            branch_if_false
        }
    }
}

// --- Helper functions -------------------------------------------------------

/// Look up the vector table address for an interrupt name on a given CPU family.
/// Returns the address where the linker should write the 2-byte target function
/// address.
fn interrupt_vector_address(cpu: &str, interrupt_name: &str) -> Option<u32> {
    match cpu {
        "mos6502" | "mos65sc02" | "ricoh2a03" | "ricoh2a07" => match interrupt_name {
            "reset" => Some(0xFFFC),
            "nmi" => Some(0xFFFA),
            "irq" => Some(0xFFFE),
            _ => None,
        },
        "wdc65c816" => match interrupt_name {
            "reset" => Some(0xFFFC),
            "nmi" => Some(0xFFEA),
            "irq" => Some(0xFFEE),
            "abort" => Some(0xFFE8),
            "cop" => Some(0xFFE4),
            "brk" => Some(0xFFE6),
            _ => None,
        },
        // Other CPU families: no vector table support yet.
        _ => None,
    }
}

/// Get a u32 value from an attribute argument by name.
fn get_attr_u32(attr: &Attribute, key: &str) -> Option<u32> {
    attr.args.iter().find_map(|arg| {
        if arg.name == key {
            let val = arg.value.trim_matches('"');
            if let Some(hex) = val.strip_prefix("0x") {
                u32::from_str_radix(hex, 16).ok()
            } else {
                val.parse::<u32>().ok()
            }
        } else {
            None
        }
    })
}

/// Compute the byte size of a type.
fn type_size(ty: &Type) -> usize {
    match ty {
        Type::Named { name } => match name.as_str() {
            "u8" | "i8" | "bool" => 1,
            "u16" | "i16" => 2,
            "u32" | "i32" => 4,
            "pointer" => 2,
            _ => 1,
        },
        Type::Array { element, size } => {
            let elem_size = type_size(element);
            if let Some(size_expr) = size {
                if let Some(s) = eval_const_expr_simple(size_expr) {
                    elem_size * s as usize
                } else {
                    0
                }
            } else {
                0
            }
        }
    }
}

/// Resolve a selector (`path::access + offset`) to a constant value.
///
/// The fully qualified name (e.g. `PPU::CNT0`) is tried first, then the
/// bare path (`PPU`). Any trailing offset access is folded into the
/// result. Returns `None` when the selector does not name a constant.
/// Fold the evaluable `Offset` accesses of a selector into a signed
/// addend. Offsets that cannot be evaluated are ignored.
fn selector_offset(accesses: &[Access], const_values: &HashMap<String, i64>) -> i64 {
    let mut offset: i64 = 0;
    for access in accesses {
        if let Access::Offset { op, value } = access {
            if let Some(v) = eval_expr(value, const_values) {
                offset = match op {
                    OffsetOp::Add => offset + v,
                    OffsetOp::Sub => offset - v,
                };
            }
        }
    }
    offset
}

/// The offset addend of an expression, if it is a selector with
/// offset accesses; otherwise zero.
fn selector_addend(expr: &Expr, const_values: &HashMap<String, i64>) -> i64 {
    match expr {
        Expr::Selector { accesses, .. } => selector_offset(accesses, const_values),
        _ => 0,
    }
}

fn resolve_selector(
    path: &[String],
    accesses: &[Access],
    const_values: &HashMap<String, i64>,
) -> Option<i64> {
    let path_name = path.join("::");
    let mut full_name = path_name.clone();
    for access in accesses {
        if let Access::ModuleAccess { name } | Access::FieldAccess { name } = access {
            full_name.push_str("::");
            full_name.push_str(name);
        }
    }
    let offset = selector_offset(accesses, const_values);
    let value = const_values.get(&full_name).or_else(|| {
        if full_name != path_name {
            const_values.get(&path_name)
        } else {
            None
        }
    })?;
    Some(value + offset)
}

/// Evaluate an expression to a constant value, using the const value table.
fn eval_expr(expr: &Expr, const_values: &HashMap<String, i64>) -> Option<i64> {
    match expr {
        Expr::Number { value } => Some(*value),
        Expr::Boolean { value } => Some(if *value { 1 } else { 0 }),
        Expr::Ident { name } => const_values.get(name).copied(),
        Expr::UnaryOp { op, operand } => {
            let v = eval_expr(operand, const_values)?;
            Some(match op {
                op_common::ast::UnaryOp::Neg => -v,
                op_common::ast::UnaryOp::Pos => v,
                op_common::ast::UnaryOp::Not => {
                    if v != 0 {
                        0
                    } else {
                        1
                    }
                }
                op_common::ast::UnaryOp::Inv => !v,
            })
        }
        Expr::BinOp { op, left, right } => {
            let l = eval_expr(left, const_values)?;
            let r = eval_expr(right, const_values)?;
            Some(match op {
                op_common::ast::BinaryOp::Or => l | r,
                op_common::ast::BinaryOp::Xor => l ^ r,
                op_common::ast::BinaryOp::And => l & r,
                op_common::ast::BinaryOp::Add => l + r,
                op_common::ast::BinaryOp::Sub => l - r,
                op_common::ast::BinaryOp::Mul => l * r,
                op_common::ast::BinaryOp::Div => {
                    if r == 0 {
                        return None;
                    }
                    l / r
                }
                op_common::ast::BinaryOp::Mod => {
                    if r == 0 {
                        return None;
                    }
                    l % r
                }
                op_common::ast::BinaryOp::Shl => l << r,
                op_common::ast::BinaryOp::Shr => l >> r,
                _ => return None,
            })
        }
        Expr::MacroCall { name, arg } => {
            let v = eval_expr(arg, const_values)?;
            Some(match name.as_str() {
                "lo" => v & 0xFF,
                "hi" => (v >> 8) & 0xFF,
                "nylo" => v & 0x0F,
                "nyhi" => (v >> 4) & 0x0F,
                _ => return None,
            })
        }
        Expr::ParenExpr { inner } => eval_expr(inner, const_values),
        // Selector like PPU::CNT0 — resolve against the const table so
        // that memory and immediate operands can use the value directly.
        Expr::Selector { path, accesses } => resolve_selector(path, accesses, const_values),
        _ => None,
    }
}

/// Evaluate a const expression without a symbol table (for type sizes).
fn eval_const_expr_simple(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Number { value } => Some(*value),
        Expr::ParenExpr { inner } => eval_const_expr_simple(inner),
        Expr::BinOp { op, left, right } => {
            let l = eval_const_expr_simple(left)?;
            let r = eval_const_expr_simple(right)?;
            Some(match op {
                op_common::ast::BinaryOp::Add => l + r,
                op_common::ast::BinaryOp::Sub => l - r,
                op_common::ast::BinaryOp::Mul => l * r,
                _ => return None,
            })
        }
        _ => None,
    }
}

/// Extract a symbol name from an expression, if it references one.
fn expr_to_symbol(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident { name } => Some(name.clone()),
        Expr::Selector { path, .. } => {
            if !path.is_empty() {
                Some(path.join("::"))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Convert a value to a byte array of the given size (little-endian).
fn val_to_bytes(val: i64, size: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(size);
    for i in 0..size {
        bytes.push(((val >> (i * 8)) & 0xFF) as u8);
    }
    bytes
}

/// Find the std crate root directory.
///
/// Searches, in order:
/// 1. The CLI include paths (`-I` / `--include`), in the order given.
/// 2. The `OP_STD_PATH` environment variable.
/// 3. The default install path `$HOME/.carts/std/src`.
///
/// Returns the first candidate directory that contains a `lib.op` file.
fn find_std_root(include_paths: &[String]) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    for path in include_paths {
        candidates.push(std::path::Path::new(path).to_path_buf());
    }

    if let Ok(env) = std::env::var("OP_STD_PATH") {
        if !env.is_empty() {
            candidates.push(std::path::Path::new(&env).to_path_buf());
        }
    }

    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(std::path::Path::new(&home).join(".carts/std/src"));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.join("lib.op").is_file())
}

/// Map a module path to its source file. A module with segments
/// `a/b/c` lives in `dir/a/b/c.op` or `dir/a/b/c/mod.op`; a module
/// with no segments lives in `dir/lib.op` or `dir/mod.op`.
fn module_file_path(dir: &std::path::Path, segments: &[String]) -> Option<std::path::PathBuf> {
    if segments.is_empty() {
        for name in ["lib.op", "mod.op"] {
            let file = dir.join(name);
            if file.is_file() {
                return Some(file);
            }
        }
        return None;
    }
    let mut base = dir.to_path_buf();
    for segment in &segments[..segments.len() - 1] {
        base.push(segment);
    }
    base.push(&segments[segments.len() - 1]);
    let name = base.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let file = base.with_file_name(format!("{name}.op"));
    if file.is_file() {
        return Some(file);
    }
    let mod_file = base.join("mod.op");
    if mod_file.is_file() {
        return Some(mod_file);
    }
    None
}

/// Return the name a declaration binds, if any.
fn decl_name(item: &Item) -> Option<&str> {
    match item {
        Item::ConstDecl { name, .. }
        | Item::VarDecl { name, .. }
        | Item::FnDecl { name, .. }
        | Item::InlineFnDecl { name, .. }
        | Item::StructDecl { name, .. }
        | Item::TypeDecl { name, .. }
        | Item::EnumDecl { name, .. } => Some(name),
        Item::ModDecl { name, .. } => Some(name),
        Item::UseDecl { .. } | Item::BlockAttribute { .. } | Item::Placement { .. } => None,
    }
}

// --- Tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::find_std_root;
    use super::Codegen;
    use super::InlineFn;
    use super::ModuleCache;
    use crate::encoding::get_full_encoding_table;
    use op_common::ast::{
        Access, EnumVariant, Expr, FnStmt, OffsetOp, Operand, PlacementArg, UseRoot, UseTail,
        UseTree,
    };
    use op_common::TargetTriplet;
    use op_diagnostics::Severity;
    use op_ir::{Section, SectionKind};
    use std::collections::HashMap;

    /// Serializes tests that mutate the process environment.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Run `f` with `OP_STD_PATH` and `HOME` pointed at locations that do
    /// not contain a std root, then restore the previous environment.
    fn with_isolated_env(tmp: &std::path::Path, f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old_op_std = std::env::var("OP_STD_PATH").ok();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("OP_STD_PATH", tmp.join("no-such-std"));
        std::env::set_var("HOME", tmp);
        f();
        match old_op_std {
            Some(value) => std::env::set_var("OP_STD_PATH", value),
            None => std::env::remove_var("OP_STD_PATH"),
        }
        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }

    /// Run `f` with `OP_STD_PATH` set to `root` and `HOME` pointed at an
    /// empty directory, then restore the previous environment. Holds
    /// the environment lock for the duration.
    fn with_std_env(root: &std::path::Path, f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old_op_std = std::env::var("OP_STD_PATH").ok();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("OP_STD_PATH", root);
        std::env::set_var("HOME", std::env::temp_dir());
        f();
        match old_op_std {
            Some(value) => std::env::set_var("OP_STD_PATH", value),
            None => std::env::remove_var("OP_STD_PATH"),
        }
        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }

    /// Write a minimal fake std crate under `std_root`.
    fn write_fake_std(std_root: &std::path::Path) {
        std::fs::create_dir_all(std_root.join("cpu")).unwrap();
        std::fs::write(std_root.join("lib.op"), "pub mod cpu;\n").unwrap();
        std::fs::write(
            std_root.join("cpu.op"),
            "const CYCLES: u8 = 1;\nmod mos6502;\npub use mos6502::*;\n",
        )
        .unwrap();
        std::fs::write(
            std_root.join("cpu/mos6502.op"),
            "enum REGS { A = 0x2000, B = 0x2001 }\ninline fn nop() {\n    nop\n}\npub use REGS::*;\n",
        )
        .unwrap();
    }

    /// Write a fake std crate mirroring the `machine/nes` layout: a
    /// module whose private `use super::` imports bind the names its
    /// inline fns reference.
    fn write_nes_std(std_root: &std::path::Path) {
        std::fs::create_dir_all(std_root.join("nes")).unwrap();
        std::fs::write(std_root.join("lib.op"), "pub mod nes;\n").unwrap();
        std::fs::write(
            std_root.join("nes.op"),
            "mod constants;\nmod macros;\npub use constants::*;\npub use macros::*;\n",
        )
        .unwrap();
        std::fs::write(
            std_root.join("nes/constants.op"),
            "const ST_VBLANK: u8 = 0x80;\n",
        )
        .unwrap();
        std::fs::write(
            std_root.join("nes/macros.op"),
            "use super::constants::*;\ninline fn vblank_on() {\n    sta ST_VBLANK\n}\n",
        )
        .unwrap();
    }

    #[test]
    fn find_std_root_returns_none_when_missing() {
        let tmp = std::env::temp_dir().join(format!("opc-find-std-root-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        with_isolated_env(&tmp, || {
            let include = vec![tmp.join("no-such-include").to_string_lossy().to_string()];
            assert_eq!(find_std_root(&include), None);
            assert_eq!(find_std_root(&[]), None);
        });
    }

    #[test]
    fn load_module_caches_parsed_module() {
        let tmp = std::env::temp_dir().join(format!("opc-module-cache-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let tmp = tmp.canonicalize().unwrap();
        let file = tmp.join("cache-test.op");
        std::fs::write(&file, "const ANSWER: u8 = 42;\n").unwrap();
        let target = TargetTriplet::parse("mos6502-nintendo-nes-ntsc").unwrap();

        let mut cache = ModuleCache::default();
        let first = cache.load_module(&file, &target, &[]).unwrap();
        assert_eq!(first.items.len(), 1);

        // Remove the file. A second load must still succeed from the cache.
        std::fs::remove_file(&file).unwrap();
        let second = cache.load_module(&file, &target, &[]).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn load_module_errors_when_file_missing() {
        let target = TargetTriplet::parse("mos6502-nintendo-nes-ntsc").unwrap();
        let mut cache = ModuleCache::default();
        let missing = std::path::Path::new("/nonexistent-opc-test/missing.op");
        assert!(cache.load_module(missing, &target, &[]).is_err());
    }

    /// Build a minimal Codegen for unit tests.
    fn test_codegen() -> Codegen {
        let target = TargetTriplet::parse("mos6502-nintendo-nes-ntsc").unwrap();
        Codegen {
            target,
            opt_level: 0,
            encoding_table: get_full_encoding_table("mos6502"),
            sections: Vec::new(),
            current_section: None,
            inline_fns: HashMap::new(),
            const_values: HashMap::new(),
            module_cache: ModuleCache::default(),
            module_path: Vec::new(),
            enum_variants: HashMap::new(),
            use_aliases: HashMap::new(),
            include_paths: Vec::new(),
            features: Vec::new(),
            current_module_dir: None,
            label_counter: 0,
            interrupt_vectors: Vec::new(),
            header: None,
            pad_byte: 0,
            source_dir: std::path::PathBuf::new(),
            diags: Vec::new(),
        }
    }

    #[test]
    fn resolve_root_converts_use_roots() {
        let mut codegen = test_codegen();

        // At the crate root: lib, self, and super all resolve to the
        // empty path.
        assert_eq!(codegen.resolve_root(&UseRoot::Lib), Vec::<String>::new());
        assert_eq!(
            codegen.resolve_root(&UseRoot::SelfMod),
            Vec::<String>::new()
        );
        assert_eq!(codegen.resolve_root(&UseRoot::Super), Vec::<String>::new());
        assert_eq!(
            codegen.resolve_root(&UseRoot::Name("std".into())),
            vec!["std".to_string()]
        );

        // Inside a nested module: self is the full stack, super drops
        // the last element.
        codegen.module_path.push("std".to_string());
        codegen.module_path.push("cpu".to_string());
        assert_eq!(
            codegen.current_module_path(),
            vec!["std".to_string(), "cpu".to_string()]
        );
        assert_eq!(
            codegen.resolve_root(&UseRoot::SelfMod),
            vec!["std".to_string(), "cpu".to_string()]
        );
        assert_eq!(
            codegen.resolve_root(&UseRoot::Super),
            vec!["std".to_string()]
        );
    }

    #[test]
    fn use_tree_resolves_std_items() {
        let tmp = std::env::temp_dir().join(format!("opc-use-tree-{}", std::process::id()));
        let std_root = tmp.join("std/src");
        write_fake_std(&std_root);

        with_std_env(&std_root, || {
            let mut codegen = test_codegen();
            let tree = UseTree::Path {
                root: UseRoot::Name("std".into()),
                segments: vec!["cpu".to_string()],
                tail: UseTail::Glob,
            };
            codegen.resolve_use_decl(&[tree]);

            assert_eq!(codegen.const_values.get("CYCLES"), Some(&1));
            assert_eq!(codegen.enum_variants.get("REGS::A"), Some(&0x2000));
            assert_eq!(codegen.const_values.get("REGS::A"), Some(&0x2000));
            // A glob of an enum also binds the variant names bare.
            assert_eq!(codegen.const_values.get("A"), Some(&0x2000));
            assert!(codegen.inline_fns.contains_key("nop"));
            assert!(codegen.diags.is_empty());
        });
    }

    #[test]
    fn use_tree_item_import_binds_single_item() {
        let tmp = std::env::temp_dir().join(format!("opc-use-tree-item-{}", std::process::id()));
        let std_root = tmp.join("std/src");
        write_fake_std(&std_root);

        with_std_env(&std_root, || {
            let mut codegen = test_codegen();
            let tree = UseTree::Path {
                root: UseRoot::Name("std".into()),
                segments: vec!["cpu".to_string(), "CYCLES".to_string()],
                tail: UseTail::Item,
            };
            codegen.resolve_use_decl(&[tree]);

            assert_eq!(codegen.const_values.get("CYCLES"), Some(&1));
            // An item import does not pull in the module's other items.
            assert!(!codegen.const_values.contains_key("A"));
            assert!(!codegen.inline_fns.contains_key("nop"));
            assert!(codegen.diags.is_empty());
        });
    }

    #[test]
    fn use_tree_records_module_aliases() {
        let mut codegen = test_codegen();
        let inner = UseTree::Path {
            root: UseRoot::Name("std".into()),
            segments: vec!["cpu".to_string()],
            tail: UseTail::Item,
        };
        let tree = UseTree::Alias {
            inner: Box::new(inner),
            alias: "c".to_string(),
        };
        codegen.resolve_use_decl(&[tree]);

        let expected = vec!["std".to_string(), "cpu".to_string()];
        assert_eq!(codegen.use_aliases.get("c"), Some(&expected));
    }

    #[test]
    fn use_tree_errors_when_std_missing() {
        let tmp = std::env::temp_dir().join(format!("opc-use-tree-nostd-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();

        with_isolated_env(&tmp, || {
            let mut codegen = test_codegen();
            let tree = UseTree::Path {
                root: UseRoot::Name("std".into()),
                segments: vec!["cpu".to_string()],
                tail: UseTail::Glob,
            };
            codegen.resolve_use_decl(&[tree]);

            assert!(codegen
                .diags
                .iter()
                .any(|d| d.severity == Severity::Error && d.code == 302));
            assert!(codegen.const_values.is_empty());
        });
    }

    /// Build a variant with an explicit numeric value.
    fn num_variant(name: &str, value: i64) -> EnumVariant {
        EnumVariant {
            name: name.to_string(),
            value: Some(Expr::Number { value }),
        }
    }

    /// Build a variant with no explicit value.
    fn implicit_variant(name: &str) -> EnumVariant {
        EnumVariant {
            name: name.to_string(),
            value: None,
        }
    }

    #[test]
    fn collect_enum_evaluates_explicit_and_implicit_values() {
        let mut codegen = test_codegen();

        // Explicit values are evaluated as written.
        codegen.collect_enum(
            "STATUS",
            &[
                num_variant("N", 0x80),
                num_variant("V", 0x40),
                num_variant("C", 0x01),
            ],
            false,
        );
        assert_eq!(codegen.const_values.get("STATUS::N"), Some(&0x80));
        assert_eq!(codegen.const_values.get("STATUS::V"), Some(&0x40));
        assert_eq!(codegen.const_values.get("STATUS::C"), Some(&0x01));
        assert_eq!(codegen.enum_variants.get("STATUS::N"), Some(&0x80));

        // Variants without a value count up from the previous variant,
        // starting at zero for the first variant.
        codegen.collect_enum(
            "OPCODE",
            &[
                implicit_variant("BRK"),
                implicit_variant("ORA"),
                implicit_variant("JMP"),
            ],
            false,
        );
        assert_eq!(codegen.const_values.get("OPCODE::BRK"), Some(&0));
        assert_eq!(codegen.const_values.get("OPCODE::ORA"), Some(&1));
        assert_eq!(codegen.const_values.get("OPCODE::JMP"), Some(&2));

        // An explicit value resets the implicit count.
        codegen.collect_enum(
            "COND",
            &[
                num_variant("plus", 0),
                implicit_variant("minus"),
                num_variant("equal", 5),
                implicit_variant("carry"),
            ],
            false,
        );
        assert_eq!(codegen.const_values.get("COND::plus"), Some(&0));
        assert_eq!(codegen.const_values.get("COND::minus"), Some(&1));
        assert_eq!(codegen.const_values.get("COND::equal"), Some(&5));
        assert_eq!(codegen.const_values.get("COND::carry"), Some(&6));

        // Glob import binds bare names, but never overwrites a name
        // that is already bound.
        codegen.const_values.insert("a".to_string(), 99);
        codegen.collect_enum(
            "REGS",
            &[implicit_variant("a"), implicit_variant("x")],
            true,
        );
        assert_eq!(codegen.const_values.get("REGS::a"), Some(&0));
        assert_eq!(codegen.const_values.get("a"), Some(&99));
        assert_eq!(codegen.const_values.get("x"), Some(&1));
        assert!(codegen.diags.is_empty());
    }

    /// Build a minimal Codegen with an active ROM section, so emitted
    /// instruction bytes and relocations can be inspected.
    fn test_codegen_with_rom_section() -> Codegen {
        let mut codegen = test_codegen();
        codegen.sections.push(Section {
            name: "rom_bank0".to_string(),
            kind: SectionKind::Rom,
            org: 0x8000,
            bank: 0,
            maxsize: 0x8000,
            symbols: Vec::new(),
            relocations: Vec::new(),
            data: Vec::new(),
        });
        codegen.current_section = Some(0);
        codegen
    }

    /// Build a selector operand from a `::`-separated name like
    /// `PPU::CNT0`.
    fn selector_operand(name: &str) -> Operand {
        let mut parts = name.split("::");
        let head = parts.next().unwrap_or_default().to_string();
        let accesses = parts
            .map(|part| Access::ModuleAccess {
                name: part.to_string(),
            })
            .collect();
        Operand::Selector {
            path: vec![head],
            accesses,
        }
    }

    #[test]
    fn selector_resolves_to_const_value() {
        let mut codegen = test_codegen_with_rom_section();
        codegen.const_values.insert("PPU::CNT0".to_string(), 0x2000);

        codegen.compile_asm("sta", &[selector_operand("PPU::CNT0")]);

        // `sta $2000` -> 8D 00 20, with no relocation.
        assert_eq!(codegen.sections[0].data, vec![0x8D, 0x00, 0x20]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    #[test]
    fn selector_falls_back_to_path_name() {
        let mut codegen = test_codegen_with_rom_section();
        // Only the bare path is a known constant.
        codegen.const_values.insert("PPU".to_string(), 0x2000);

        codegen.compile_asm("sta", &[selector_operand("PPU::CNT0")]);

        assert_eq!(codegen.sections[0].data, vec![0x8D, 0x00, 0x20]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    #[test]
    fn selector_emits_relocation_when_unknown() {
        let mut codegen = test_codegen_with_rom_section();

        codegen.compile_asm("sta", &[selector_operand("PPU::CNT0")]);

        // Placeholder bytes plus an Abs16 relocation against `PPU`.
        assert_eq!(codegen.sections[0].data, vec![0x8D, 0x00, 0x00]);
        assert_eq!(codegen.sections[0].relocations.len(), 1);
        assert_eq!(codegen.sections[0].relocations[0].symbol, "PPU");
    }

    /// The parser delivers selectors as `Expr::Selector` inside a
    /// memory operand, so verify that path resolves constants too.
    #[test]
    fn selector_in_memory_operand_resolves_to_const() {
        let mut codegen = test_codegen_with_rom_section();
        codegen.const_values.insert("PPU::CNT0".to_string(), 0x2000);

        let expr = Expr::Selector {
            path: vec!["PPU".to_string()],
            accesses: vec![Access::ModuleAccess {
                name: "CNT0".to_string(),
            }],
        };
        let operand = Operand::MemoryOperand {
            mode_prefix: None,
            expr,
            index_reg: None,
        };
        codegen.compile_asm("sta", &[operand]);

        // `sta $2000` -> 8D 00 20, with no relocation.
        assert_eq!(codegen.sections[0].data, vec![0x8D, 0x00, 0x20]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// A trailing offset access folds into the resolved constant.
    #[test]
    fn selector_with_offset_folds_into_value() {
        let mut codegen = test_codegen_with_rom_section();
        codegen.const_values.insert("PPU::CNT0".to_string(), 0x2000);

        let expr = Expr::Selector {
            path: vec!["PPU".to_string()],
            accesses: vec![
                Access::ModuleAccess {
                    name: "CNT0".to_string(),
                },
                Access::Offset {
                    op: OffsetOp::Add,
                    value: Expr::Number { value: 1 },
                },
            ],
        };
        let operand = Operand::MemoryOperand {
            mode_prefix: None,
            expr,
            index_reg: None,
        };
        codegen.compile_asm("sta", &[operand]);

        // `sta $2001` -> 8D 01 20.
        assert_eq!(codegen.sections[0].data, vec![0x8D, 0x01, 0x20]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// Expand an inline fn with two parameters and check the emitted
    /// bytes against a hand-assembled equivalent.
    #[test]
    fn inline_fn_substitutes_two_params() {
        let mut codegen = test_codegen_with_rom_section();
        // inline fn copy(src, dst) { lda src; sta dst }
        codegen.inline_fns.insert(
            "copy".to_string(),
            InlineFn {
                params: vec!["src".to_string(), "dst".to_string()],
                body: vec![
                    FnStmt::AsmStmt {
                        opcode: "lda".to_string(),
                        operands: vec![Operand::MemoryOperand {
                            mode_prefix: None,
                            expr: Expr::Ident {
                                name: "src".to_string(),
                            },
                            index_reg: None,
                        }],
                    },
                    FnStmt::AsmStmt {
                        opcode: "sta".to_string(),
                        operands: vec![Operand::MemoryOperand {
                            mode_prefix: None,
                            expr: Expr::Ident {
                                name: "dst".to_string(),
                            },
                            index_reg: None,
                        }],
                    },
                ],
            },
        );

        codegen.compile_fn_call(
            "copy",
            &[
                Expr::Number { value: 0x2000 },
                Expr::Number { value: 0x4000 },
            ],
        );

        // Hand-assembled equivalent: lda $2000 / sta $4000.
        assert_eq!(
            codegen.sections[0].data,
            vec![0xAD, 0x00, 0x20, 0x8D, 0x00, 0x40]
        );
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// A nested inline call in the substituted body resolves during
    /// the compile pass.
    #[test]
    fn inline_fn_expands_nested_calls() {
        let mut codegen = test_codegen_with_rom_section();
        // inline fn pause() { nop }
        codegen.inline_fns.insert(
            "pause".to_string(),
            InlineFn {
                params: Vec::new(),
                body: vec![FnStmt::AsmStmt {
                    opcode: "nop".to_string(),
                    operands: Vec::new(),
                }],
            },
        );
        // inline fn step(value) { pause(); lda value }
        codegen.inline_fns.insert(
            "step".to_string(),
            InlineFn {
                params: vec!["value".to_string()],
                body: vec![
                    FnStmt::FnCall {
                        name: "pause".to_string(),
                        args: Vec::new(),
                    },
                    FnStmt::AsmStmt {
                        opcode: "lda".to_string(),
                        operands: vec![Operand::MemoryOperand {
                            mode_prefix: None,
                            expr: Expr::Ident {
                                name: "value".to_string(),
                            },
                            index_reg: None,
                        }],
                    },
                ],
            },
        );

        codegen.compile_fn_call("step", &[Expr::Number { value: 0x42 }]);

        // Hand-assembled equivalent: nop / lda $42.
        assert_eq!(codegen.sections[0].data, vec![0xEA, 0xA5, 0x42]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// Substitution reaches into nested call arguments.
    #[test]
    fn inline_fn_substitutes_nested_call_args() {
        let mut codegen = test_codegen_with_rom_section();
        // inline fn write(value) { lda value }
        codegen.inline_fns.insert(
            "write".to_string(),
            InlineFn {
                params: vec!["value".to_string()],
                body: vec![FnStmt::AsmStmt {
                    opcode: "lda".to_string(),
                    operands: vec![Operand::MemoryOperand {
                        mode_prefix: None,
                        expr: Expr::Ident {
                            name: "value".to_string(),
                        },
                        index_reg: None,
                    }],
                }],
            },
        );
        // inline fn pipe(value) { write(value) }
        codegen.inline_fns.insert(
            "pipe".to_string(),
            InlineFn {
                params: vec!["value".to_string()],
                body: vec![FnStmt::FnCall {
                    name: "write".to_string(),
                    args: vec![Expr::Ident {
                        name: "value".to_string(),
                    }],
                }],
            },
        );

        codegen.compile_fn_call("pipe", &[Expr::Number { value: 0x42 }]);

        // pipe(0x42) -> write(0x42) -> lda $42.
        assert_eq!(codegen.sections[0].data, vec![0xA5, 0x42]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// Parse `source` and run the two-pass module walk on it.
    fn walk_parsed_source(source: &str) -> (Codegen, Vec<op_diagnostics::Diagnostic>) {
        let (ast, diags) =
            crate::parser::parse_source("multi-pass.op", source, "mos6502-nintendo-nes-ntsc", &[]);
        assert!(diags.is_empty(), "parse diagnostics: {diags:?}");
        let mut codegen = test_codegen_with_rom_section();
        codegen.walk_module(&ast.root);
        (codegen, diags)
    }

    #[test]
    fn multi_pass_resolves_const_declared_after_fn() {
        let (codegen, _) = walk_parsed_source(
            "fn early() {\n    lda ANSWER\n    rts\n}\nconst ANSWER: u8 = 42;\n",
        );
        // lda $42 (zero-page) / rts, no relocation for ANSWER.
        assert_eq!(codegen.sections[0].data, vec![0xA5, 0x2A, 0x60]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    #[test]
    fn multi_pass_resolves_use_declared_after_fn() {
        let tmp = std::env::temp_dir().join(format!("opc-multi-pass-use-{}", std::process::id()));
        let std_root = tmp.join("std/src");
        write_fake_std(&std_root);

        with_std_env(&std_root, || {
            let (codegen, _) =
                walk_parsed_source("fn early() {\n    lda CYCLES\n    rts\n}\nuse std::cpu::*;\n");
            // CYCLES is 1; lda $1 (zero-page) / rts, no relocation.
            assert_eq!(codegen.sections[0].data, vec![0xA5, 0x01, 0x60]);
            assert!(codegen.sections[0].relocations.is_empty());
            assert!(codegen.diags.is_empty());
        });
    }

    #[test]
    fn multi_pass_const_references_earlier_const() {
        let (codegen, _) = walk_parsed_source(
            "fn early() {\n    lda B\n    rts\n}\nconst A: u8 = 40;\nconst B: u8 = A + 2;\n",
        );
        // B evaluates to 42 against the collected value of A.
        assert_eq!(codegen.sections[0].data, vec![0xA5, 0x2A, 0x60]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    #[test]
    fn use_tree_resolves_super_in_std_modules() {
        let tmp = std::env::temp_dir().join(format!("opc-super-use-{}", std::process::id()));
        let std_root = tmp.join("std/src");
        write_nes_std(&std_root);

        with_std_env(&std_root, || {
            // The user imports the macros module directly, so the
            // constants names can only arrive through macros.op's
            // private `use super::constants::*;`.
            let (codegen, _) = walk_parsed_source(
                "use std::nes::macros::*;\nfn main() {\n    vblank_on()\n    rts\n}\n",
            );
            // vblank_on expands to `sta ST_VBLANK` -> sta $80
            // (zero-page) / rts, with no relocation and no
            // diagnostics.
            assert_eq!(codegen.sections[0].data, vec![0x85, 0x80, 0x60]);
            assert!(codegen.sections[0].relocations.is_empty());
            assert!(codegen.diags.is_empty());
        });
    }

    #[test]
    fn module_cache_records_source_directory() {
        let tmp = std::env::temp_dir().join(format!("opc-module-dir-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let tmp = tmp.canonicalize().unwrap();
        let file = tmp.join("dirtest.op");
        std::fs::write(&file, "const X: u8 = 1;\n").unwrap();
        let target = TargetTriplet::parse("mos6502-nintendo-nes-ntsc").unwrap();

        let mut cache = ModuleCache::default();
        cache.load_module(&file, &target, &[]).unwrap();

        assert_eq!(cache.dir_of(&file), Some(&tmp));
        assert_eq!(cache.dir_of(&tmp.join("missing.op")), None);
    }

    #[test]
    fn locate_bytes_resolves_relative_to_source_dir() {
        let tmp = std::env::temp_dir().join(format!("opc-locate-src-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("blob.bin"), [0x11u8, 0x22, 0x33]).unwrap();

        let mut codegen = test_codegen_with_rom_section();
        codegen.source_dir = tmp.clone();
        let arg = PlacementArg::String_ {
            value: "\"blob.bin\"".to_string(),
        };
        codegen.handle_placement("locate_bytes", &arg);

        assert_eq!(codegen.sections[0].data, vec![0x11, 0x22, 0x33]);
        assert!(codegen.diags.is_empty());
    }

    #[test]
    fn locate_bytes_in_std_module_resolves_relative_to_std_file() {
        let tmp = std::env::temp_dir().join(format!("opc-locate-std-{}", std::process::id()));
        let std_root = tmp.join("std/src");
        std::fs::create_dir_all(&std_root).unwrap();
        std::fs::write(std_root.join("lib.op"), "pub mod res;\n").unwrap();
        std::fs::write(std_root.join("res.op"), "locate_bytes!(\"data.bin\");\n").unwrap();
        std::fs::write(std_root.join("data.bin"), [0xDEu8, 0xAD, 0xBE, 0xEF]).unwrap();

        with_std_env(&std_root, || {
            // An active section is required for placement macros in
            // imported modules to emit. source_dir is empty, so the
            // bytes can only come from the std module's own
            // directory.
            let mut codegen = test_codegen_with_rom_section();
            let tree = UseTree::Path {
                root: UseRoot::Name("std".into()),
                segments: vec!["res".to_string()],
                tail: UseTail::Glob,
            };
            codegen.resolve_use_decl(&[tree]);

            assert_eq!(codegen.sections[0].data, vec![0xDE, 0xAD, 0xBE, 0xEF]);
            assert!(codegen.diags.is_empty());
        });
    }
}
