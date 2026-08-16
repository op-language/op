; locals.scm for the Op language
; Provides scope and definition/reference info for Neovim features
; (go-to-definition, rename, highlight-current-scope, etc.)

; --- Definitions -----------------------------------------------------------

(const_decl name: (identifier) @local.definition.constant)
(var_decl name: (identifier) @local.definition.var)
(field name: (identifier) @local.definition.field)
(param_list (identifier) @local.definition.parameter)

(fn_decl name: (identifier) @local.definition.function)
(inline_fn_decl name: (identifier) @local.definition.function)

(struct_decl name: (identifier) @local.definition.type)
(type_decl name: (identifier) @local.definition.type)
(enum_decl name: (identifier) @local.definition.type)

(enum_variant name: (identifier) @local.definition.constant)

(mod_decl name: (identifier) @local.definition.namespace)

(label_def) @local.definition

; --- References ------------------------------------------------------------

(identifier) @local.reference
(path) @local.reference
(selector) @local.reference
(register_ref) @local.reference
(label_ref) @local.reference
(use_simple) @local.reference
(use_path_root) @local.reference

; --- Scopes ----------------------------------------------------------------

(mod_decl) @local.scope
(fn_body) @local.scope
(block) @local.scope
(source_file) @local.scope