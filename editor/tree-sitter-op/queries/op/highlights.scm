; highlights.scm for the Op language
; Maps tree-sitter nodes to Neovim capture names.

; --- Comments --------------------------------------------------------------

(module_doc_comment) @comment.documentation
(doc_comment) @comment.documentation
(line_comment) @comment
(block_comment) @comment

((line_comment) @comment.todo
  (#match? @comment.todo "(TODO|FIXME|HACK|NOTE|XXX)"))

; --- Keywords --------------------------------------------------------------

[
  "fn" "inline" "noreturn" "volatile"
  "struct" "type" "enum" "const" "mod" "use" "pub"
  "lib" "self" "super"
  "if" "else" "while" "do" "loop" "switch" "case" "default"
  "near" "far" "as"
] @keyword

(return_stmt) @keyword

; --- Booleans --------------------------------------------------------------

"true" @boolean
"false" @boolean

; --- Types -----------------------------------------------------------------

(primitive_type) @type.builtin

((identifier) @type
  (#lua-match? @type "^[A-Z][A-Za-z0-9_]*$"))

; --- Numbers ---------------------------------------------------------------

(number) @number

; --- Strings ---------------------------------------------------------------

(string) @string
(string_escape) @string.escape

; --- Attributes ------------------------------------------------------------

(attribute) @attribute
(attr_path (identifier) @attribute)

; --- Immediate prefix ------------------------------------------------------

(immediate "#" @operator)

; --- Operators -------------------------------------------------------------

[
  "::" "." "+" "-" "*" "/" "%" "~" "!" "&" "^" "|"
  "<<" ">>" ">" "<" ">=" "<=" "==" "!=" "="
] @operator

[
  "(" ")" "{" "}" "[" "]"
] @punctuation.bracket

[
  ":" "," ";"
] @punctuation.delimiter

; --- Labels ----------------------------------------------------------------

(label_def) @label
(label_ref) @label

; --- Compile-time macros ---------------------------------------------------

(macro_call
  macro: _ @function.macro)

(include_macro_call
  macro: _ @function.macro)

; --- Opcodes ---------------------------------------------------------------

(opcode) @keyword

; --- Register references ---------------------------------------------------

(register_ref) @variable.builtin

; --- Condition keywords ----------------------------------------------------

(condition_keyword) @keyword.conditional
(modifier) @keyword.conditional

; --- Mode prefixes ---------------------------------------------------------

(mode_prefix) @keyword.modifier

; --- Declarations ----------------------------------------------------------

(const_decl name: (identifier) @constant)
(var_decl name: (identifier) @variable)
(fn_decl name: (identifier) @function)
(inline_fn_decl name: (identifier) @function)
(struct_decl name: (identifier) @type)
(type_decl name: (identifier) @type)
(enum_decl name: (identifier) @type)
(mod_decl name: (identifier) @namespace)
(use_decl (use_tree) @namespace)
(use_alias alias: (identifier) @namespace)
(use_path_root) @namespace

; --- Enum variants ---------------------------------------------------------

(enum_variant name: (identifier) @constant)

; --- Struct fields ---------------------------------------------------------

(field name: (identifier) @variable.member)

; --- Paths -----------------------------------------------------------------

(path) @namespace

; --- Identifiers (fallback) ------------------------------------------------

(identifier) @variable