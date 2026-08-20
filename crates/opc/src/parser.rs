//! Stage 2: parser.
//!
//! The parser reads the token stream from the lexer and builds a full AST.
//! It evaluates `#[cfg]` attributes, evaluates const expressions, and
//! resolves `mod` file declarations. The parser is a hand-written
//! recursive-descent parser.

use anyhow::Result;
use op_common::{
    ast::{
        Access, AttrArg, Attribute, BinaryOp, BranchHint, Condition, EnumVariant, Expr, Field,
        FnStmt, InitValue, Item, Module, OffsetOp, Operand, PlacementArg, SwitchCase, Type,
        UnaryOp, UseRoot, UseTail, UseTree,
    },
    AstFile, TargetTriplet, Token, TokenStream,
};
use op_diagnostics::{Diagnostic, Severity};
use std::path::Path;

use crate::cli::OpcArgs;
use crate::lexer;

// --- Entry points -----------------------------------------------------------

/// Run the parser stage when the `--parse` flag is set, or no-op otherwise.
///
/// When the `--parse` flag is set, the input is a `.opx` token-stream file
/// produced by the `--lex` stage. The parser deserializes the token stream
/// and builds an AST from it.
pub fn run(args: &OpcArgs) -> Result<()> {
    if !args.parse {
        return Ok(());
    }
    let target = args.target.as_deref().unwrap_or("");
    let json = std::fs::read_to_string(&args.input.input)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", args.input.input))?;
    let stream: TokenStream = op_common::from_json(&json)?;
    // The token stream's `file` field holds the original source path. Use it
    // so the AST carries the source path forward to the codegen, which needs
    // it to resolve `locate_bytes!`/`locate_str!` paths and the standard
    // library relative to the source directory.
    let (ast, diags) = parse_token_stream(&stream.file.clone(), stream, target, &args.features);
    let has_errors = diags.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        for d in &diags {
            d.print(None);
        }
        anyhow::bail!("parser errors in {}", args.input.input);
    }
    let out = op_common::to_json(&ast)?;
    match &args.output {
        Some(path) => std::fs::write(path, out)?,
        None => println!("{out}"),
    }
    Ok(())
}

/// Parse a source file into an [`AstFile`].
pub fn parse_file(path: &str, target: &str, features: &[String]) -> Result<AstFile> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path))?;
    let (ast, diags) = parse_source(path, &source, target, features);
    let has_errors = diags.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        for d in &diags {
            d.print(None);
        }
        anyhow::bail!("parser errors in {}", path);
    }
    Ok(ast)
}

/// Parse a source string into an [`AstFile`] and a list of diagnostics.
pub fn parse_source(
    file: &str,
    source: &str,
    target: &str,
    features: &[String],
) -> (AstFile, Vec<Diagnostic>) {
    let (token_stream, lex_diags) = lexer::lex_source(file, source);
    parse_token_stream_with_diags(file, token_stream, lex_diags, target, features)
}

/// Parse a serialized [`TokenStream`] into an [`AstFile`] and a list of
/// diagnostics. This is the entry point for the `--parse` stage when it
/// reads a `.opx` file. The token stream already contains the lexer
/// diagnostics, if any, from the `--lex` stage.
pub fn parse_token_stream(
    file: &str,
    stream: TokenStream,
    target: &str,
    features: &[String],
) -> (AstFile, Vec<Diagnostic>) {
    parse_token_stream_with_diags(file, stream, Vec::new(), target, features)
}

/// Shared body of [`parse_source`] and [`parse_token_stream`].
///
/// `lex_diags` are diagnostics produced by the lexer stage. When the parser
/// reads a `.opx` file, the lexer diagnostics are not re-emitted, so the
/// caller passes an empty vector.
fn parse_token_stream_with_diags(
    file: &str,
    stream: TokenStream,
    lex_diags: Vec<Diagnostic>,
    target: &str,
    features: &[String],
) -> (AstFile, Vec<Diagnostic>) {
    let triplet = TargetTriplet::parse(target).unwrap_or(TargetTriplet {
        cpu: String::new(),
        manufacturer: String::new(),
        machine: String::new(),
        variant: String::new(),
    });

    let mut parser = Parser {
        tokens: stream.tokens,
        pos: 0,
        file: file.to_string(),
        dir: Path::new(file)
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf(),
        triplet,
        features: features.to_vec(),
        diags: lex_diags,
    };

    let module = parser.parse_module();

    (
        AstFile {
            version: 1,
            target: target.to_string(),
            file: file.to_string(),
            root: module,
        },
        parser.diags,
    )
}

// --- Parser struct ----------------------------------------------------------

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    file: String,
    dir: std::path::PathBuf,
    triplet: TargetTriplet,
    features: Vec<String>,
    diags: Vec<Diagnostic>,
}

// --- Token helpers ----------------------------------------------------------

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> &str {
        self.tokens
            .get(self.pos)
            .map(|t| t.kind.as_str())
            .unwrap_or("EOF")
    }

    fn peek_value(&self) -> &str {
        self.tokens
            .get(self.pos)
            .map(|t| t.value.as_str())
            .unwrap_or("")
    }

    fn advance(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn at_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn check(&self, kind: &str) -> bool {
        self.peek_kind() == kind
    }

    fn check_value(&self, val: &str) -> bool {
        self.peek_value() == val
    }

    fn expect(&mut self, kind: &str) -> Result<Token, ()> {
        if self.check(kind) {
            Ok(self.advance().unwrap())
        } else {
            let tok = self.peek();
            let (line, col) = tok.map(|t| (t.line, t.col)).unwrap_or((0, 0));
            self.diags.push(Diagnostic::error(
                200,
                &self.file,
                line,
                col,
                format!("expected {} but got {}", kind, self.peek_kind()),
            ));
            Err(())
        }
    }

    fn expect_value(&mut self, kind: &str, value: &str) -> Result<Token, ()> {
        if self.check(kind) && self.check_value(value) {
            Ok(self.advance().unwrap())
        } else {
            let tok = self.peek();
            let (line, col) = tok.map(|t| (t.line, t.col)).unwrap_or((0, 0));
            self.diags.push(Diagnostic::error(
                200,
                &self.file,
                line,
                col,
                format!(
                    "expected {} '{}' but got {} '{}'",
                    kind,
                    value,
                    self.peek_kind(),
                    self.peek_value()
                ),
            ));
            Err(())
        }
    }

    /// Consume a semicolon if present.
    fn optional_semicolon(&mut self) {
        if self.check("Op_semicolon") {
            self.advance();
        }
    }

    /// Check if the current token is a keyword.
    fn check_kw(&self, kw: &str) -> bool {
        self.peek_kind() == format!("Kw_{}", kw)
    }

    /// Check if the current token is a condition keyword.
    fn check_cond(&self) -> bool {
        self.peek_kind().starts_with("Cond_")
    }

    /// Check if the current token is a condition modifier.
    fn check_mod(&self) -> bool {
        self.peek_kind().starts_with("Mod_")
    }

    /// Check if the current token is a mode prefix.
    fn check_mode(&self) -> bool {
        self.peek_kind().starts_with("Mode_")
    }

    fn error(&mut self, code: u32, msg: impl Into<String>) {
        let tok = self.peek();
        let (line, col) = tok.map(|t| (t.line, t.col)).unwrap_or((0, 0));
        self.diags
            .push(Diagnostic::error(code, &self.file, line, col, msg));
    }
}

// --- Module parsing ---------------------------------------------------------

impl Parser {
    fn parse_module(&mut self) -> Module {
        let name = Path::new(&self.file)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.file.clone());
        let mut module = Module::new(name);

        while !self.at_eof() {
            // Collect attributes before an item.
            let attrs = self.parse_attributes();

            // If no attributes and no token, we're done.
            if attrs.is_empty() && self.at_eof() {
                break;
            }

            // Check for cfg-guarded items.
            if let Some(cfg_attr) = find_cfg(&attrs) {
                if !self.eval_cfg(&cfg_attr) {
                    // Skip the item that follows the cfg attribute.
                    self.skip_item();
                    continue;
                }
            }

            // Check if the next token is a block: #[attr] { items }
            // The last attribute starts the block.
            if self.check("Op_lbrace") {
                // Block attribute: use the last attribute as the block attr.
                if attrs.len() == 1 {
                    let attr = attrs.into_iter().next().unwrap();
                    self.advance(); // {
                    let mut items = Vec::new();
                    while !self.check("Op_rbrace") && !self.at_eof() {
                        let inner_attrs = self.parse_attributes();
                        if let Some(cfg_attr) = find_cfg(&inner_attrs) {
                            if !self.eval_cfg(&cfg_attr) {
                                self.skip_item();
                                continue;
                            }
                        }
                        if let Some(item) = self.parse_item_with_attrs(inner_attrs) {
                            items.push(item);
                        } else if !self.at_eof() {
                            self.error(
                                201,
                                format!("unexpected token in block: {}", self.peek_kind()),
                            );
                            self.advance();
                        }
                    }
                    let _ = self.expect("Op_rbrace"); // }
                    module.items.push(Item::BlockAttribute { attr, items });
                    continue;
                } else {
                    // Multiple attributes before a block — use the last one
                    // as the block attr, store the rest as standalone.
                    for attr in &attrs[..attrs.len() - 1] {
                        module.items.push(Item::BlockAttribute {
                            attr: attr.clone(),
                            items: Vec::new(),
                        });
                    }
                    let block_attr = attrs.last().unwrap().clone();
                    self.advance(); // {
                    let mut items = Vec::new();
                    while !self.check("Op_rbrace") && !self.at_eof() {
                        let inner_attrs = self.parse_attributes();
                        if let Some(cfg_attr) = find_cfg(&inner_attrs) {
                            if !self.eval_cfg(&cfg_attr) {
                                self.skip_item();
                                continue;
                            }
                        }
                        if let Some(item) = self.parse_item_with_attrs(inner_attrs) {
                            items.push(item);
                        } else if !self.at_eof() {
                            self.error(
                                201,
                                format!("unexpected token in block: {}", self.peek_kind()),
                            );
                            self.advance();
                        }
                    }
                    let _ = self.expect("Op_rbrace"); // }
                    module.items.push(Item::BlockAttribute {
                        attr: block_attr,
                        items,
                    });
                    continue;
                }
            }

            // If we have attributes but no item follows, store each as a
            // standalone BlockAttribute with empty items.
            if self.at_eof() || self.check("Op_hash") {
                for attr in attrs {
                    module.items.push(Item::BlockAttribute {
                        attr,
                        items: Vec::new(),
                    });
                }
                continue;
            }

            if let Some(item) = self.parse_item_with_attrs(attrs) {
                module.items.push(item);
            } else if self.at_eof() {
                break;
            } else {
                // Unexpected token — emit error and skip.
                self.error(
                    201,
                    format!(
                        "unexpected token: {} '{}'",
                        self.peek_kind(),
                        self.peek_value()
                    ),
                );
                self.advance();
            }
        }

        module
    }

    /// Skip an item (used when cfg drops it). Parses the item and
    /// discards the result. This ensures the parser consumes exactly
    /// one item, including any nested blocks.
    fn skip_item(&mut self) {
        // Collect any additional attributes on the item.
        let attrs = self.parse_attributes();
        // Parse and discard the item.
        let _ = self.parse_item_with_attrs(attrs);
    }

    fn parse_item_with_attrs(&mut self, attrs: Vec<Attribute>) -> Option<Item> {
        let kind = self.peek_kind().to_string();

        // Block attributes: #[rom(...)] { ... }
        if kind == "Op_hash" {
            // This shouldn't happen since we collected attributes above,
            // but handle it just in case.
            return self.parse_block_attribute_item(attrs);
        }

        // Placement macros: locate_bytes!(...), locate_fn!(...)
        if kind.starts_with("Include_") {
            return Some(self.parse_placement(attrs));
        }

        // Keywords
        if let Some(kw) = kind.strip_prefix("Kw_") {
            return match kw {
                "const" => Some(self.parse_const_decl(attrs)),
                "fn" => Some(self.parse_fn_decl(false, attrs)),
                "noreturn" => Some(self.parse_fn_decl(true, attrs)),
                "inline" => Some(self.parse_inline_fn_decl(attrs)),
                "struct" => Some(self.parse_struct_decl(attrs)),
                "type" => Some(self.parse_type_decl(attrs)),
                "enum" => Some(self.parse_enum_decl(attrs)),
                "mod" => Some(self.parse_mod_decl(attrs)),
                "use" => Some(self.parse_use_decl(attrs)),
                "pub" => Some(self.parse_pub_item(attrs)),
                "volatile" => Some(self.parse_var_decl(attrs)),
                _ => {
                    self.error(201, format!("unexpected keyword: {}", kw));
                    self.advance();
                    None
                }
            };
        }

        // Variable declaration: starts with an IDENT
        if kind == "IDENT" {
            return Some(self.parse_var_decl(attrs));
        }

        None
    }

    fn parse_pub_item(&mut self, attrs: Vec<Attribute>) -> Item {
        self.advance(); // consume 'pub'
        let kind = self.peek_kind().to_string();
        if let Some(kw) = kind.strip_prefix("Kw_") {
            match kw {
                "mod" => {
                    let mut item = self.parse_mod_decl(attrs);
                    if let Item::ModDecl { is_pub, .. } = &mut item {
                        *is_pub = true;
                    }
                    item
                }
                "use" => {
                    let mut item = self.parse_use_decl(attrs);
                    if let Item::UseDecl { is_pub, .. } = &mut item {
                        *is_pub = true;
                    }
                    item
                }
                "fn" => {
                    let item = self.parse_fn_decl(false, attrs);
                    // pub fn is public
                    item
                }
                _ => {
                    self.error(201, format!("unexpected keyword after 'pub': {}", kw));
                    self.advance();
                    Item::UseDecl {
                        is_pub: false,
                        trees: vec![],
                    }
                }
            }
        } else {
            self.error(201, "expected keyword after 'pub'");
            Item::UseDecl {
                is_pub: false,
                trees: vec![],
            }
        }
    }

    fn parse_block_attribute_item(&mut self, attrs: Vec<Attribute>) -> Option<Item> {
        // Parse the attribute
        let attr = self.parse_single_attribute()?;

        // Check for block: #[attr] { items }
        if self.check("Op_lbrace") {
            self.advance(); // {
            let mut items = Vec::new();
            while !self.check("Op_rbrace") && !self.at_eof() {
                let inner_attrs = self.parse_attributes();
                if let Some(cfg_attr) = find_cfg(&inner_attrs) {
                    if !self.eval_cfg(&cfg_attr) {
                        self.skip_item();
                        continue;
                    }
                }
                if let Some(item) = self.parse_item_with_attrs(inner_attrs) {
                    items.push(item);
                } else if !self.at_eof() {
                    self.error(
                        201,
                        format!("unexpected token in block: {}", self.peek_kind()),
                    );
                    self.advance();
                }
            }
            let _ = self.expect("Op_rbrace"); // }
            Some(Item::BlockAttribute { attr, items })
        } else {
            // Not a block — treat as item with attributes
            let mut all_attrs = attrs;
            all_attrs.push(attr);
            self.parse_item_with_attrs(all_attrs)
        }
    }
}

// --- Attribute parsing ------------------------------------------------------

impl Parser {
    /// Parse zero or more attributes before an item.
    fn parse_attributes(&mut self) -> Vec<Attribute> {
        let mut attrs = Vec::new();
        while self.check("Op_hash") {
            if let Some(attr) = self.parse_single_attribute() {
                attrs.push(attr);
            }
        }
        attrs
    }

    /// Parse a single `#[path(args)]` attribute.
    fn parse_single_attribute(&mut self) -> Option<Attribute> {
        if !self.check("Op_hash") {
            return None;
        }
        self.advance(); // #
        if self.expect("Op_lbracket").is_err() {
            return None;
        }

        // Parse the attribute path: identifier (:: identifier)*
        let mut path = String::new();
        if let Some(tok) = self.advance() {
            path = tok.value;
        }
        while self.check("Op_colon_colon") {
            self.advance();
            if let Some(tok) = self.advance() {
                path.push_str("::");
                path.push_str(&tok.value);
            }
        }

        // Parse optional arguments: ( arg, arg, ... )
        let mut args = Vec::new();
        if self.check("Op_lparen") {
            self.advance(); // (
            while !self.check("Op_rparen") && !self.at_eof() {
                args.push(self.parse_attr_arg());
                if self.check("Op_comma") {
                    self.advance();
                }
            }
            let _ = self.expect("Op_rparen"); // )
        }

        let _ = self.expect("Op_rbracket"); // ]

        Some(Attribute { path, args })
    }

    fn parse_attr_arg(&mut self) -> AttrArg {
        // An attr arg is either:
        //   identifier
        //   literal (NUMBER, STRING, Kw_true, Kw_false)
        //   identifier = literal

        let tok = self.peek().cloned();
        let (name, has_eq) = if let Some(ref t) = tok {
            match t.kind.as_str() {
                "IDENT" | "OPCODE" | "Type_u8" | "Type_i8" | "Type_u16" | "Type_i16"
                | "Type_u32" | "Type_i32" | "Type_bool" | "Type_pointer" => {
                    let name = t.value.clone();
                    self.advance();
                    if self.check("Op_assign") {
                        self.advance();
                        (name, true)
                    } else {
                        return AttrArg {
                            name,
                            value: String::new(),
                        };
                    }
                }
                "NUMBER" | "STRING" => {
                    let value = t.value.clone();
                    self.advance();
                    return AttrArg {
                        name: String::new(),
                        value,
                    };
                }
                "Kw_true" | "Kw_false" => {
                    let value = t.value.clone();
                    self.advance();
                    return AttrArg {
                        name: String::new(),
                        value,
                    };
                }
                _ => {
                    self.advance();
                    (t.value.clone(), false)
                }
            }
        } else {
            (String::new(), false)
        };

        if !has_eq {
            return AttrArg {
                name,
                value: String::new(),
            };
        }

        // We consumed the identifier and the '=', now get the value.
        let value = if let Some(tok) = self.peek() {
            let v = tok.value.clone();
            self.advance();
            v
        } else {
            String::new()
        };

        AttrArg { name, value }
    }
}

// --- Module-level declaration parsing ---------------------------------------

impl Parser {
    fn parse_const_decl(&mut self, attrs: Vec<Attribute>) -> Item {
        self.advance(); // 'const'
        let name = self.advance().map(|t| t.value).unwrap_or_default();
        let _ = self.expect("Op_colon"); // :
        let ty = self.parse_type();
        let _ = self.expect("Op_assign"); // =
        let value = self.parse_expr();
        self.optional_semicolon();

        let evaluated_value = eval_const_expr(&value);

        Item::ConstDecl {
            name,
            ty,
            value,
            evaluated_value,
            attributes: attrs,
        }
    }

    fn parse_var_decl(&mut self, attrs: Vec<Attribute>) -> Item {
        let is_volatile = if self.check_kw("volatile") {
            self.advance();
            true
        } else {
            false
        };

        let name = self.advance().map(|t| t.value).unwrap_or_default();
        let _ = self.expect("Op_colon"); // :
        let ty = self.parse_type();

        let array_dim = if self.check("Op_lbracket") {
            self.advance(); // [
            if self.check("Op_rbracket") {
                self.advance(); // ]
                None
            } else {
                let expr = self.parse_expr();
                let _ = self.expect("Op_rbracket"); // ]
                Some(expr)
            }
        } else {
            None
        };

        let addr_binding = if self.check("Op_colon") {
            self.advance(); // :
            Some(self.parse_expr())
        } else {
            None
        };

        let init = if self.check("Op_assign") {
            self.advance(); // =
            Some(self.parse_init_value())
        } else {
            None
        };

        self.optional_semicolon();

        Item::VarDecl {
            name,
            is_volatile,
            ty,
            array_dim,
            addr_binding,
            init,
            attributes: attrs,
        }
    }

    fn parse_init_value(&mut self) -> InitValue {
        if self.check("STRING") {
            let value = self.advance().map(|t| t.value).unwrap_or_default();
            return InitValue::String_ { value };
        }
        if self.check("Op_lbrace") {
            self.advance(); // {
            let mut items = Vec::new();
            while !self.check("Op_rbrace") && !self.at_eof() {
                items.push(self.parse_init_value());
                if self.check("Op_comma") {
                    self.advance();
                }
            }
            let _ = self.expect("Op_rbrace"); // }
            return InitValue::InitList { items };
        }
        InitValue::Expr {
            value: self.parse_expr(),
        }
    }

    fn parse_fn_decl(&mut self, is_noreturn: bool, attrs: Vec<Attribute>) -> Item {
        if is_noreturn {
            self.advance(); // 'noreturn'
        }
        self.advance(); // 'fn'
        let name = self.advance().map(|t| t.value).unwrap_or_default();
        let _ = self.expect("Op_lparen"); // (
        let _ = self.expect("Op_rparen"); // )
        let body = self.parse_fn_body();

        Item::FnDecl {
            name,
            is_noreturn,
            attributes: attrs,
            body,
        }
    }

    fn parse_inline_fn_decl(&mut self, attrs: Vec<Attribute>) -> Item {
        self.advance(); // 'inline'
        self.advance(); // 'fn'
        let name = self.advance().map(|t| t.value).unwrap_or_default();
        let _ = self.expect("Op_lparen"); // (

        let mut params = Vec::new();
        if !self.check("Op_rparen") {
            if let Some(tok) = self.advance() {
                params.push(tok.value);
            }
            while self.check("Op_comma") {
                self.advance();
                if let Some(tok) = self.advance() {
                    params.push(tok.value);
                }
            }
        }
        let _ = self.expect("Op_rparen"); // )
        let body = self.parse_fn_body();

        Item::InlineFnDecl {
            name,
            params,
            attributes: attrs,
            body,
        }
    }

    fn parse_struct_decl(&mut self, attrs: Vec<Attribute>) -> Item {
        self.advance(); // 'struct'
        let name = self.advance().map(|t| t.value).unwrap_or_default();
        let _ = self.expect("Op_lbrace"); // {

        let mut fields = Vec::new();
        while !self.check("Op_rbrace") && !self.at_eof() {
            let is_volatile = if self.check_kw("volatile") {
                self.advance();
                true
            } else {
                false
            };
            let field_name = self.advance().map(|t| t.value).unwrap_or_default();
            let _ = self.expect("Op_colon"); // :
            let ty = self.parse_type();
            let array_dim = if self.check("Op_lbracket") {
                self.advance(); // [
                if self.check("Op_rbracket") {
                    self.advance();
                    None
                } else {
                    let expr = self.parse_expr();
                    let _ = self.expect("Op_rbracket");
                    Some(expr)
                }
            } else {
                None
            };
            fields.push(Field {
                is_volatile,
                name: field_name,
                ty,
                array_dim,
            });
            if self.check("Op_comma") {
                self.advance();
            }
        }
        let _ = self.expect("Op_rbrace"); // }

        Item::StructDecl {
            name,
            fields,
            attributes: attrs,
        }
    }

    fn parse_type_decl(&mut self, attrs: Vec<Attribute>) -> Item {
        self.advance(); // 'type'
        let name = self.advance().map(|t| t.value).unwrap_or_default();
        let _ = self.expect("Op_assign"); // =
        let ty = self.parse_type();
        self.optional_semicolon();

        Item::TypeDecl {
            name,
            ty,
            attributes: attrs,
        }
    }

    fn parse_enum_decl(&mut self, attrs: Vec<Attribute>) -> Item {
        self.advance(); // 'enum'
        let name = self.advance().map(|t| t.value).unwrap_or_default();
        let _ = self.expect("Op_lbrace"); // {

        let mut variants = Vec::new();
        while !self.check("Op_rbrace") && !self.at_eof() {
            let vname = self.advance().map(|t| t.value).unwrap_or_default();
            let value = if self.check("Op_assign") {
                self.advance(); // =
                Some(self.parse_expr())
            } else {
                None
            };
            variants.push(EnumVariant { name: vname, value });
            if self.check("Op_comma") {
                self.advance();
            }
        }
        let _ = self.expect("Op_rbrace"); // }

        Item::EnumDecl {
            name,
            variants,
            attributes: attrs,
        }
    }

    fn parse_mod_decl(&mut self, attrs: Vec<Attribute>) -> Item {
        self.advance(); // 'mod'
        let name = self.advance().map(|t| t.value).unwrap_or_default();

        if self.check("Op_semicolon") {
            self.advance(); // ;
                            // File module — try to resolve.
            let resolved = self.resolve_mod_file(&name);
            Item::ModDecl {
                name,
                is_pub: false,
                body: None,
                resolved: resolved.map(Box::new),
                attributes: attrs,
            }
        } else if self.check("Op_lbrace") {
            self.advance(); // {
            let mut items = Vec::new();
            while !self.check("Op_rbrace") && !self.at_eof() {
                let inner_attrs = self.parse_attributes();
                if let Some(cfg_attr) = find_cfg(&inner_attrs) {
                    if !self.eval_cfg(&cfg_attr) {
                        self.skip_item();
                        continue;
                    }
                }
                if let Some(item) = self.parse_item_with_attrs(inner_attrs) {
                    items.push(item);
                } else if !self.at_eof() {
                    self.error(
                        201,
                        format!("unexpected token in mod body: {}", self.peek_kind()),
                    );
                    self.advance();
                }
            }
            let _ = self.expect("Op_rbrace"); // }
            Item::ModDecl {
                name,
                is_pub: false,
                body: Some(items),
                resolved: None,
                attributes: attrs,
            }
        } else {
            self.error(201, "expected ';' or '{' after mod name");
            Item::ModDecl {
                name,
                is_pub: false,
                body: None,
                resolved: None,
                attributes: attrs,
            }
        }
    }

    fn parse_use_decl(&mut self, attrs: Vec<Attribute>) -> Item {
        self.advance(); // 'use'
        let mut trees = Vec::new();

        // Parse one or more use trees separated by commas.
        // Note: use_decl attributes are collected but UseDecl has no
        // attributes field — we attach them via a wrapper if needed.
        // For now, we ignore attributes on use decls.
        let _ = attrs;

        trees.push(self.parse_use_tree());
        while self.check("Op_comma") {
            self.advance();
            trees.push(self.parse_use_tree());
        }
        self.optional_semicolon();

        Item::UseDecl {
            is_pub: false,
            trees,
        }
    }

    fn parse_use_tree(&mut self) -> UseTree {
        // Parse the root.
        let root = if self.check("Kw_lib") {
            self.advance();
            UseRoot::Lib
        } else if self.check("Kw_self") {
            self.advance();
            UseRoot::SelfMod
        } else if self.check("Kw_super") {
            self.advance();
            UseRoot::Super
        } else {
            let name = self.advance().map(|t| t.value).unwrap_or_default();
            UseRoot::Name(name)
        };

        // Parse intermediate segments.
        let mut segments = Vec::new();
        while self.check("Op_colon_colon") {
            // Look ahead: is this followed by { or * (glob/group) or an identifier?
            let next_next = self
                .tokens
                .get(self.pos + 1)
                .map(|t| t.kind.as_str())
                .unwrap_or("");
            if next_next == "Op_lbrace" || next_next == "Op_star" {
                break;
            }
            self.advance(); // ::
            if let Some(tok) = self.advance() {
                segments.push(tok.value);
            } else {
                break;
            }
        }

        // Parse the tail.
        let tail = if self.check("Op_colon_colon") {
            self.advance(); // ::
            if self.check("Op_star") {
                self.advance(); // *
                UseTail::Glob
            } else if self.check("Op_lbrace") {
                self.advance(); // {
                let mut group = Vec::new();
                while !self.check("Op_rbrace") && !self.at_eof() {
                    group.push(self.parse_use_tree());
                    if self.check("Op_comma") {
                        self.advance();
                    }
                }
                let _ = self.expect("Op_rbrace"); // }
                UseTail::Group(group)
            } else {
                // Last segment is an item name.
                let name = self.advance().map(|t| t.value).unwrap_or_default();
                segments.push(name);
                UseTail::Item
            }
        } else {
            UseTail::Item
        };

        let tree = UseTree::Path {
            root,
            segments,
            tail,
        };

        // Check for `as alias`.
        if self.check_kw("as") {
            self.advance(); // 'as'
            let alias = self.advance().map(|t| t.value).unwrap_or_default();
            UseTree::Alias {
                inner: Box::new(tree),
                alias,
            }
        } else {
            tree
        }
    }

    fn parse_placement(&mut self, attrs: Vec<Attribute>) -> Item {
        let macro_tok = self.advance().unwrap_or(Token {
            kind: "IDENT".to_string(),
            value: String::new(),
            line: 0,
            col: 0,
        });
        // The lexer already consumed the '!' as part of the macro token.
        // The token value is the macro name without the '!'.
        let macro_name = macro_tok.value.clone();

        let _ = self.expect("Op_lparen"); // (
        let argument = if self.check("STRING") {
            let value = self.advance().map(|t| t.value).unwrap_or_default();
            PlacementArg::String_ { value }
        } else {
            // Parse a path: ident (:: ident)*
            let mut segments = Vec::new();
            if let Some(tok) = self.advance() {
                segments.push(tok.value);
            }
            while self.check("Op_colon_colon") {
                self.advance();
                if let Some(tok) = self.advance() {
                    segments.push(tok.value);
                }
            }
            PlacementArg::Path { segments }
        };
        let _ = self.expect("Op_rparen"); // )
        self.optional_semicolon();

        Item::Placement {
            macro_name,
            argument,
            attributes: attrs,
        }
    }

    fn parse_type(&mut self) -> Type {
        if self.check("Op_lbracket") {
            self.advance(); // [
            let element = self.parse_type();
            let size = if self.check("Op_semicolon") {
                self.advance();
                Some(self.parse_expr())
            } else {
                None
            };
            let _ = self.expect("Op_rbracket"); // ]
            Type::Array {
                element: Box::new(element),
                size,
            }
        } else {
            let name = self.advance().map(|t| t.value).unwrap_or_default();
            Type::Named { name }
        }
    }
}

// --- Function body parsing --------------------------------------------------

impl Parser {
    fn parse_fn_body(&mut self) -> Vec<FnStmt> {
        let mut stmts = Vec::new();
        if self.expect("Op_lbrace").is_err() {
            return stmts;
        }
        while !self.check("Op_rbrace") && !self.at_eof() {
            if let Some(stmt) = self.parse_fn_stmt() {
                stmts.push(stmt);
            } else if !self.at_eof() {
                self.error(
                    202,
                    format!("unexpected token in function body: {}", self.peek_kind()),
                );
                self.advance();
            }
        }
        let _ = self.expect("Op_rbrace"); // }
        stmts
    }

    fn parse_fn_stmt(&mut self) -> Option<FnStmt> {
        let kind = self.peek_kind().to_string();

        // Label: LABEL_DEF followed by a statement
        if kind == "LABEL_DEF" {
            let label_tok = self.advance().unwrap();
            let label_name = label_tok
                .value
                .trim_start_matches('\'')
                .trim_end_matches(':')
                .to_string();
            let stmt = self.parse_fn_stmt()?;
            return Some(FnStmt::Label {
                name: label_name,
                stmt: Box::new(stmt),
            });
        }

        // Return
        if self.check_kw("return") {
            self.advance();
            return Some(FnStmt::ReturnStmt);
        }

        // If
        if self.check_kw("if") {
            return Some(self.parse_if_stmt());
        }

        // While
        if self.check_kw("while") {
            return Some(self.parse_while_stmt());
        }

        // Do
        if self.check_kw("do") {
            return Some(self.parse_do_while_stmt());
        }

        // Loop
        if self.check_kw("loop") {
            return Some(self.parse_loop_stmt());
        }

        // Switch
        if self.check_kw("switch") {
            return Some(self.parse_switch_stmt());
        }

        // Assembly statement: OPCODE
        if kind == "OPCODE" {
            return Some(self.parse_asm_stmt());
        }

        // Label reference as a statement (e.g., jmp 'loop)
        // — this is handled as part of assembly operands, not as a statement.

        // Function call or variable declaration: starts with IDENT
        if kind == "IDENT" {
            // Look ahead: if the next token is '(' it's a function call.
            if self.tokens.get(self.pos + 1).map(|t| t.kind.as_str()) == Some("Op_lparen") {
                return Some(self.parse_fn_call_stmt());
            }
            // Otherwise it's a variable declaration.
            let attrs = self.parse_attributes();
            let var = self.parse_var_decl(attrs);
            return Some(FnStmt::VarDeclStmt {
                decl: Box::new(var),
            });
        }

        // Volatile variable declaration
        if self.check_kw("volatile") {
            let attrs = self.parse_attributes();
            let var = self.parse_var_decl(attrs);
            return Some(FnStmt::VarDeclStmt {
                decl: Box::new(var),
            });
        }

        None
    }

    fn parse_asm_stmt(&mut self) -> FnStmt {
        let opcode_tok = self.advance();
        let opcode = opcode_tok
            .as_ref()
            .map(|t| t.value.clone())
            .unwrap_or_default();
        let opcode_line = opcode_tok.map(|t| t.line).unwrap_or(0);
        let mut operands = Vec::new();
        while !self.at_eof() {
            let kind = self.peek_kind();
            // Stop at tokens that end the statement or start a new one.
            if kind == "Op_rbrace"
                || kind == "Op_semicolon"
                || kind.starts_with("Kw_")
                || kind == "LABEL_DEF"
                || kind == "OPCODE"
            {
                break;
            }
            // Stop when the next token starts on a later line than the
            // opcode. The lexer discards newlines, so a line break is the
            // statement boundary for assembly instructions. This prevents
            // a function call on the next line from being consumed as a
            // second operand of the opcode on this line.
            if let Some(tok) = self.peek() {
                if tok.line > opcode_line {
                    break;
                }
            }
            if let Some(operand) = self.parse_operand() {
                operands.push(operand);
            } else {
                break;
            }
        }
        self.optional_semicolon();
        FnStmt::AsmStmt { opcode, operands }
    }

    fn parse_operand(&mut self) -> Option<Operand> {
        let kind = self.peek_kind().to_string();

        // Immediate: # expr
        if kind == "Op_hash" {
            self.advance();
            let value = self.parse_expr();
            return Some(Operand::Immediate { value });
        }

        // Label reference: 'ident
        if kind == "LABEL_REF" {
            let tok = self.advance().unwrap();
            let name = tok.value.trim_start_matches('\'').to_string();
            return Some(Operand::LabelRef { name });
        }

        // Mode prefix (zp, abs, rel, ind, idx, ind_l, ind_idx)
        let mode_prefix = if self.check_mode() {
            let tok = self.advance().unwrap();
            Some(tok.value)
        } else {
            None
        };

        // Register reference: cpu::ident
        if self.check("IDENT") && self.check_value("cpu") {
            // Check if next is ::
            if self.tokens.get(self.pos + 1).map(|t| t.kind.as_str()) == Some("Op_colon_colon") {
                self.advance(); // cpu
                self.advance(); // ::
                let reg_name = self.advance().map(|t| t.value).unwrap_or_default();
                return Some(Operand::RegisterRef { name: reg_name });
            }
        }

        // Parenthesized memory operand: (expr) [, index_reg]
        if self.check("Op_lparen") {
            self.advance(); // (
            let expr = self.parse_expr();

            // Could be (expr, index_reg) or (expr) or (expr), index_reg
            let index_reg = if self.check("Op_comma") {
                self.advance();
                let idx = self.parse_index_reg();
                let _ = self.expect("Op_rparen");
                idx
            } else {
                let _ = self.expect("Op_rparen");
                // Check for trailing , index_reg
                if self.check("Op_comma") {
                    self.advance();
                    self.parse_index_reg()
                } else {
                    None
                }
            };

            return Some(Operand::MemoryOperand {
                mode_prefix,
                expr,
                index_reg,
                is_indirect: true,
            });
        }

        // Expression-based memory operand: expr [, index_reg]
        let expr = self.parse_expr();

        // Check for , index_reg
        if self.check("Op_comma") {
            self.advance();
            let index_reg = self.parse_index_reg();
            return Some(Operand::MemoryOperand {
                mode_prefix,
                expr,
                index_reg,
                is_indirect: false,
            });
        }

        // Check for selector accesses (::ident, .ident, +expr, -expr)
        if self.check("Op_colon_colon")
            || self.check("Op_dot")
            || self.check("Op_plus")
            || self.check("Op_minus")
        {
            let (path, accesses) = self.parse_selector_continuation(expr);
            return Some(Operand::Selector { path, accesses });
        }

        // Plain expression — wrap as a selector with no accesses, or as
        // a memory operand with no mode prefix.
        Some(Operand::MemoryOperand {
            mode_prefix,
            expr,
            index_reg: None,
            is_indirect: false,
        })
    }

    fn parse_index_reg(&mut self) -> Option<String> {
        // cpu::ident or just ident
        if self.check("IDENT")
            && self.check_value("cpu")
            && self.tokens.get(self.pos + 1).map(|t| t.kind.as_str()) == Some("Op_colon_colon")
        {
            self.advance(); // cpu
            self.advance(); // ::
            let name = self.advance().map(|t| t.value).unwrap_or_default();
            return Some(format!("cpu::{}", name));
        }
        if self.check("IDENT") {
            let name = self.advance().map(|t| t.value).unwrap_or_default();
            return Some(name);
        }
        None
    }

    /// Given an expression that was already parsed (as the first part of
    /// a selector), continue parsing ::ident, .ident, +expr, -expr accesses.
    fn parse_selector_continuation(&mut self, first: Expr) -> (Vec<String>, Vec<Access>) {
        // Extract the path from the first expression.
        let (path, mut accesses) = match first {
            Expr::Ident { name } => (vec![name], Vec::new()),
            Expr::Selector { path, accesses } => (path, accesses),
            _ => (vec![], Vec::new()),
        };

        while self.check("Op_colon_colon")
            || self.check("Op_dot")
            || self.check("Op_plus")
            || self.check("Op_minus")
        {
            if self.check("Op_colon_colon") {
                self.advance();
                let name = self.advance().map(|t| t.value).unwrap_or_default();
                accesses.push(Access::ModuleAccess { name });
            } else if self.check("Op_dot") {
                self.advance();
                let name = self.advance().map(|t| t.value).unwrap_or_default();
                accesses.push(Access::FieldAccess { name });
            } else if self.check("Op_plus") {
                self.advance();
                let value = self.parse_primary_expr();
                accesses.push(Access::Offset {
                    op: OffsetOp::Add,
                    value,
                });
            } else if self.check("Op_minus") {
                self.advance();
                let value = self.parse_primary_expr();
                accesses.push(Access::Offset {
                    op: OffsetOp::Sub,
                    value,
                });
            }
        }

        (path, accesses)
    }

    fn parse_if_stmt(&mut self) -> FnStmt {
        self.advance(); // 'if'
        let _ = self.expect("Op_lparen"); // (
        let branch_hint = self.parse_branch_hint();
        let condition = self.parse_condition();
        let _ = self.expect("Op_rparen"); // )

        let then_block = self.parse_block_or_stmt();
        let else_block = if self.check_kw("else") {
            self.advance();
            Some(self.parse_block_or_stmt())
        } else {
            None
        };

        FnStmt::IfStmt {
            branch_hint,
            condition,
            then_block,
            else_block,
        }
    }

    fn parse_while_stmt(&mut self) -> FnStmt {
        self.advance(); // 'while'
        let _ = self.expect("Op_lparen");
        let branch_hint = self.parse_branch_hint();
        let condition = self.parse_condition();
        let _ = self.expect("Op_rparen");
        let body = self.parse_block_or_stmt();
        FnStmt::WhileStmt {
            branch_hint,
            condition,
            body,
        }
    }

    fn parse_do_while_stmt(&mut self) -> FnStmt {
        self.advance(); // 'do'
        let body = self.parse_block_or_stmt();
        let _ = self.expect_value("Kw_while", "while"); // 'while'
        let _ = self.expect("Op_lparen");
        let branch_hint = self.parse_branch_hint();
        let condition = self.parse_condition();
        let _ = self.expect("Op_rparen");
        FnStmt::DoWhileStmt {
            body,
            branch_hint,
            condition,
        }
    }

    fn parse_loop_stmt(&mut self) -> FnStmt {
        self.advance(); // 'loop'
        let body = self.parse_block_or_stmt();
        FnStmt::LoopStmt { body }
    }

    fn parse_switch_stmt(&mut self) -> FnStmt {
        self.advance(); // 'switch'
        let _ = self.expect("Op_lparen"); // (
                                          // Expect cpu::ident
        let register = if self.check("IDENT") && self.check_value("cpu") {
            self.advance(); // cpu
            self.advance(); // ::
            self.advance().map(|t| t.value).unwrap_or_default()
        } else {
            self.advance().map(|t| t.value).unwrap_or_default()
        };
        let _ = self.expect("Op_rparen"); // )
        let _ = self.expect("Op_lbrace"); // {

        let mut cases = Vec::new();
        while !self.check("Op_rbrace") && !self.at_eof() {
            if self.check_kw("case") {
                self.advance(); // 'case'
                let expr = self.parse_expr();
                let body = self.parse_block_or_stmt();
                cases.push(SwitchCase::Case { expr, body });
            } else if self.check_kw("default") {
                self.advance(); // 'default'
                let body = self.parse_block_or_stmt();
                cases.push(SwitchCase::Default { body });
            } else {
                self.error(
                    203,
                    format!(
                        "expected 'case' or 'default' in switch, got {}",
                        self.peek_kind()
                    ),
                );
                break;
            }
        }
        let _ = self.expect("Op_rbrace"); // }

        FnStmt::SwitchStmt { register, cases }
    }

    fn parse_fn_call_stmt(&mut self) -> FnStmt {
        let name = self.advance().map(|t| t.value).unwrap_or_default();
        let _ = self.expect("Op_lparen"); // (
        let mut args = Vec::new();
        if !self.check("Op_rparen") {
            args.push(self.parse_expr());
            while self.check("Op_comma") {
                self.advance();
                args.push(self.parse_expr());
            }
        }
        let _ = self.expect("Op_rparen"); // )
        FnStmt::FnCall { name, args }
    }

    fn parse_branch_hint(&mut self) -> Option<BranchHint> {
        if self.check_kw("near") {
            self.advance();
            Some(BranchHint::Near)
        } else if self.check_kw("far") {
            self.advance();
            Some(BranchHint::Far)
        } else {
            None
        }
    }

    fn parse_condition(&mut self) -> Condition {
        let mut modifiers = Vec::new();
        while self.check_mod() {
            let tok = self.advance().unwrap();
            // Strip the "Mod_" prefix.
            let mod_name = tok.value;
            modifiers.push(mod_name);
        }
        let keyword = if self.check_cond() {
            self.advance().map(|t| t.value).unwrap_or_default()
        } else {
            self.error(
                204,
                format!("expected condition keyword, got {}", self.peek_kind()),
            );
            String::new()
        };
        Condition { modifiers, keyword }
    }

    /// Parse a block `{ stmts }` or a single statement.
    fn parse_block_or_stmt(&mut self) -> Vec<FnStmt> {
        if self.check("Op_lbrace") {
            return self.parse_fn_body();
        }
        // Single statement
        if let Some(stmt) = self.parse_fn_stmt() {
            vec![stmt]
        } else {
            Vec::new()
        }
    }
}

// --- Expression parsing (precedence climbing) --------------------------------

impl Parser {
    fn parse_expr(&mut self) -> Expr {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Expr {
        let mut left = self.parse_xor_expr();
        while self.check("Op_pipe") {
            self.advance();
            let right = self.parse_xor_expr();
            left = Expr::BinOp {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_xor_expr(&mut self) -> Expr {
        let mut left = self.parse_and_expr();
        while self.check("Op_caret") {
            self.advance();
            let right = self.parse_and_expr();
            left = Expr::BinOp {
                op: BinaryOp::Xor,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_and_expr(&mut self) -> Expr {
        let mut left = self.parse_eq_expr();
        while self.check("Op_amp") {
            self.advance();
            let right = self.parse_eq_expr();
            left = Expr::BinOp {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_eq_expr(&mut self) -> Expr {
        let mut left = self.parse_cmp_expr();
        loop {
            let op = if self.check("Op_eq") {
                BinaryOp::Eq
            } else if self.check("Op_ne") {
                BinaryOp::Ne
            } else {
                break;
            };
            self.advance();
            let right = self.parse_cmp_expr();
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_cmp_expr(&mut self) -> Expr {
        let mut left = self.parse_shift_expr();
        loop {
            let op = if self.check("Op_lt") {
                BinaryOp::Lt
            } else if self.check("Op_gt") {
                BinaryOp::Gt
            } else if self.check("Op_le") {
                BinaryOp::Le
            } else if self.check("Op_ge") {
                BinaryOp::Ge
            } else {
                break;
            };
            self.advance();
            let right = self.parse_shift_expr();
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_shift_expr(&mut self) -> Expr {
        let mut left = self.parse_add_expr();
        loop {
            let op = if self.check("Op_shl") {
                BinaryOp::Shl
            } else if self.check("Op_shr") {
                BinaryOp::Shr
            } else {
                break;
            };
            self.advance();
            let right = self.parse_add_expr();
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_add_expr(&mut self) -> Expr {
        let mut left = self.parse_mul_expr();
        loop {
            let op = if self.check("Op_plus") {
                BinaryOp::Add
            } else if self.check("Op_minus") {
                BinaryOp::Sub
            } else {
                break;
            };
            self.advance();
            let right = self.parse_mul_expr();
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_mul_expr(&mut self) -> Expr {
        let mut left = self.parse_unary_expr();
        loop {
            let op = if self.check("Op_star") {
                BinaryOp::Mul
            } else if self.check("Op_slash") {
                BinaryOp::Div
            } else if self.check("Op_percent") {
                BinaryOp::Mod
            } else {
                break;
            };
            self.advance();
            let right = self.parse_unary_expr();
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        left
    }

    fn parse_unary_expr(&mut self) -> Expr {
        let op = if self.check("Op_tilde") {
            Some(UnaryOp::Inv)
        } else if self.check("Op_bang") {
            Some(UnaryOp::Not)
        } else if self.check("Op_minus") {
            Some(UnaryOp::Neg)
        } else if self.check("Op_plus") {
            Some(UnaryOp::Pos)
        } else {
            None
        };

        if let Some(op) = op {
            self.advance();
            let operand = self.parse_unary_expr();
            return Expr::UnaryOp {
                op,
                operand: Box::new(operand),
            };
        }

        self.parse_primary_expr()
    }

    fn parse_primary_expr(&mut self) -> Expr {
        let kind = self.peek_kind().to_string();

        // Number
        if kind == "NUMBER" {
            let tok = self.advance().unwrap();
            let value = parse_number(&tok.value);
            return Expr::Number { value };
        }

        // String
        if kind == "STRING" {
            let tok = self.advance().unwrap();
            let value = tok.value.trim_matches('"').to_string();
            return Expr::String_ { value };
        }

        // Boolean: true / false
        if self.check_kw("true") {
            self.advance();
            return Expr::Boolean { value: true };
        }
        if self.check_kw("false") {
            self.advance();
            return Expr::Boolean { value: false };
        }

        // Parenthesized expression
        if self.check("Op_lparen") {
            self.advance();
            let inner = self.parse_expr();
            let _ = self.expect("Op_rparen");
            return Expr::ParenExpr {
                inner: Box::new(inner),
            };
        }

        // Compile-time macro call: lo!(expr), hi!(expr), etc.
        if kind.starts_with("Macro_") {
            let tok = self.advance().unwrap();
            let macro_name = tok.value;
            let _ = self.expect("Op_lparen");
            let arg = self.parse_expr();
            let _ = self.expect("Op_rparen");
            return Expr::MacroCall {
                name: macro_name,
                arg: Box::new(arg),
            };
        }

        // Identifier — could be a selector, path, or function call
        if kind == "IDENT"
            || kind.starts_with("Type_")
            || kind.starts_with("Cond_")
            || kind.starts_with("Mode_")
        {
            let tok = self.advance().unwrap();
            let name = tok.value;
            let path = vec![name.clone()];
            let mut accesses = Vec::new();

            // Parse ::ident, .ident, +expr, -expr
            while self.check("Op_colon_colon") || self.check("Op_dot") {
                if self.check("Op_colon_colon") {
                    self.advance();
                    let seg = self.advance().map(|t| t.value).unwrap_or_default();
                    // Check if this is a function call: ident::ident(...)
                    // For now, treat ::ident as module access.
                    accesses.push(Access::ModuleAccess { name: seg });
                } else if self.check("Op_dot") {
                    self.advance();
                    let seg = self.advance().map(|t| t.value).unwrap_or_default();
                    accesses.push(Access::FieldAccess { name: seg });
                }
            }

            // Check for offset: +expr or -expr
            while self.check("Op_plus") || self.check("Op_minus") {
                if self.check("Op_plus") {
                    self.advance();
                    let value = self.parse_primary_expr();
                    accesses.push(Access::Offset {
                        op: OffsetOp::Add,
                        value,
                    });
                } else {
                    self.advance();
                    let value = self.parse_primary_expr();
                    accesses.push(Access::Offset {
                        op: OffsetOp::Sub,
                        value,
                    });
                }
            }

            // Check for function call: ident(args)
            if self.check("Op_lparen") {
                self.advance();
                let mut args = Vec::new();
                if !self.check("Op_rparen") {
                    args.push(self.parse_expr());
                    while self.check("Op_comma") {
                        self.advance();
                        args.push(self.parse_expr());
                    }
                }
                let _ = self.expect("Op_rparen");
                return Expr::FnCall { name, args };
            }

            if !accesses.is_empty() || path.len() > 1 {
                return Expr::Selector { path, accesses };
            }

            return Expr::Ident { name };
        }

        // Fallback
        self.error(
            205,
            format!(
                "unexpected token in expression: {} '{}'",
                kind,
                self.peek_value()
            ),
        );
        self.advance();
        Expr::Number { value: 0 }
    }
}

// --- cfg evaluation ---------------------------------------------------------

impl Parser {
    /// Evaluate a cfg attribute against the target triplet and features.
    fn eval_cfg(&self, attr: &Attribute) -> bool {
        // The cfg attribute path is "cfg". The args contain the predicate.
        if attr.path != "cfg" {
            return true; // Not a cfg attribute, always include.
        }
        if attr.args.is_empty() {
            return true;
        }
        // The first arg is the predicate key=value or a combinator.
        let arg = &attr.args[0];
        self.eval_cfg_arg(arg)
    }

    fn eval_cfg_arg(&self, arg: &AttrArg) -> bool {
        // Simple key=value: name = "value"
        if !arg.name.is_empty() && !arg.value.is_empty() {
            let key = arg.name.as_str();
            let val = arg.value.trim_matches('"');
            return match key {
                "target" => self.triplet.as_str() == val,
                "cpu" => self.triplet.cpu == val,
                "manufacturer" => self.triplet.manufacturer == val,
                "machine" => self.triplet.machine == val,
                "variant" => self.triplet.variant == val,
                "feature" => self.features.iter().any(|f| f == val),
                _ => false,
            };
        }
        // The value alone could be a combinator name like "all", "any", "not".
        // But the lexer doesn't parse cfg predicates structurally — it just
        // tokenizes the attribute. The AttrArg has name="" and value=the
        // identifier for positional args.
        // For now, we only support simple key=value cfg predicates.
        // Combinators (all/any/not) require deeper attribute parsing which
        // is out of scope for this phase.
        true
    }
}

// --- Const expression evaluation --------------------------------------------

/// Evaluate a constant expression and return its value, if possible.
fn eval_const_expr(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Number { value } => Some(*value),
        Expr::Boolean { value } => Some(if *value { 1 } else { 0 }),
        Expr::UnaryOp { op, operand } => {
            let v = eval_const_expr(operand)?;
            Some(match op {
                UnaryOp::Neg => -v,
                UnaryOp::Pos => v,
                UnaryOp::Not => {
                    if v != 0 {
                        0
                    } else {
                        1
                    }
                }
                UnaryOp::Inv => !v,
            })
        }
        Expr::BinOp { op, left, right } => {
            let l = eval_const_expr(left)?;
            let r = eval_const_expr(right)?;
            Some(match op {
                BinaryOp::Or => l | r,
                BinaryOp::Xor => l ^ r,
                BinaryOp::And => l & r,
                BinaryOp::Eq => {
                    if l == r {
                        1
                    } else {
                        0
                    }
                }
                BinaryOp::Ne => {
                    if l != r {
                        1
                    } else {
                        0
                    }
                }
                BinaryOp::Lt => {
                    if l < r {
                        1
                    } else {
                        0
                    }
                }
                BinaryOp::Gt => {
                    if l > r {
                        1
                    } else {
                        0
                    }
                }
                BinaryOp::Le => {
                    if l <= r {
                        1
                    } else {
                        0
                    }
                }
                BinaryOp::Ge => {
                    if l >= r {
                        1
                    } else {
                        0
                    }
                }
                BinaryOp::Shl => l << r,
                BinaryOp::Shr => l >> r,
                BinaryOp::Add => l + r,
                BinaryOp::Sub => l - r,
                BinaryOp::Mul => l * r,
                BinaryOp::Div => {
                    if r == 0 {
                        return None;
                    }
                    l / r
                }
                BinaryOp::Mod => {
                    if r == 0 {
                        return None;
                    }
                    l % r
                }
            })
        }
        Expr::MacroCall { name, arg } => {
            let v = eval_const_expr(arg)?;
            Some(match name.as_str() {
                "lo" => v & 0xFF,
                "hi" => (v >> 8) & 0xFF,
                "nylo" => v & 0x0F,
                "nyhi" => (v >> 4) & 0x0F,
                "sizeof" => return None, // sizeof requires type info
                _ => return None,
            })
        }
        Expr::ParenExpr { inner } => eval_const_expr(inner),
        _ => None,
    }
}

// --- Mod file resolution ----------------------------------------------------

impl Parser {
    /// Try to find and parse a sub-module file.
    fn resolve_mod_file(&mut self, name: &str) -> Option<Module> {
        // Look for name.op in the same directory.
        let file_path = self.dir.join(format!("{}.op", name));
        let dir_path = self.dir.join(name).join("mod.op");

        let path = if file_path.exists() {
            file_path
        } else if dir_path.exists() {
            dir_path
        } else {
            // File not found — this is not an error at parse time.
            // The mod declaration is still recorded without a resolved module.
            return None;
        };

        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return None,
        };

        let path_str = path.to_string_lossy().to_string();
        let (token_stream, _lex_diags) = lexer::lex_source(&path_str, &source);

        let mut sub_parser = Parser {
            tokens: token_stream.tokens,
            pos: 0,
            file: path_str,
            dir: path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            triplet: self.triplet.clone(),
            features: self.features.clone(),
            diags: Vec::new(),
        };

        let module = sub_parser.parse_module();

        // Merge sub-parser diagnostics into our diagnostics.
        self.diags.extend(sub_parser.diags);

        Some(module)
    }
}

// --- Helpers ----------------------------------------------------------------

/// Find a cfg attribute in a list of attributes.
fn find_cfg(attrs: &[Attribute]) -> Option<Attribute> {
    attrs.iter().find(|a| a.path == "cfg").cloned()
}

/// Parse a number literal string into an i64.
fn parse_number(s: &str) -> i64 {
    if let Some(hex) = s.strip_prefix("0x") {
        i64::from_str_radix(hex, 16).unwrap_or(0)
    } else if let Some(bin) = s.strip_prefix('%') {
        i64::from_str_radix(bin, 2).unwrap_or(0)
    } else {
        s.parse::<i64>().unwrap_or(0)
    }
}
