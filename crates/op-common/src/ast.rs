//! AST node types for the parser stage.
//!
//! The parser emits the [`Module`] described in the technical design section
//! "Stage 2: parser". The node kinds match the normalized LR(1) grammar in
//! the language specification. The types in this file represent every
//! grammar node: module items, function body statements, expressions,
//! operands, conditions, types, attributes, and const-evaluated values.

use serde::{Deserialize, Serialize};

use crate::envelope::Envelope;

// --- Envelope and module ----------------------------------------------------

/// The `.opa` post-parser AST envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstFile {
    pub version: u32,
    pub target: String,
    /// Path of the root source file. Used by the codegen to resolve
    /// `locate_bytes!` and `locate_str!` paths relative to the source
    /// directory. Empty when the AST was not parsed from a file.
    #[serde(default)]
    pub file: String,
    pub root: Module,
}

/// The root module of an Op program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    pub kind: String,
    pub name: String,
    pub items: Vec<Item>,
}

impl Module {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            kind: "Module".to_string(),
            name: name.into(),
            items: Vec::new(),
        }
    }
}

// --- Module-level items -----------------------------------------------------

/// A top-level item inside a module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Item {
    ConstDecl {
        name: String,
        ty: Type,
        value: Expr,
        evaluated_value: Option<i64>,
        attributes: Vec<Attribute>,
    },
    VarDecl {
        name: String,
        is_volatile: bool,
        ty: Type,
        array_dim: Option<Expr>,
        addr_binding: Option<Expr>,
        init: Option<InitValue>,
        attributes: Vec<Attribute>,
    },
    FnDecl {
        name: String,
        is_noreturn: bool,
        attributes: Vec<Attribute>,
        body: Vec<FnStmt>,
    },
    InlineFnDecl {
        name: String,
        params: Vec<String>,
        attributes: Vec<Attribute>,
        body: Vec<FnStmt>,
    },
    StructDecl {
        name: String,
        fields: Vec<Field>,
        attributes: Vec<Attribute>,
    },
    TypeDecl {
        name: String,
        ty: Type,
        attributes: Vec<Attribute>,
    },
    EnumDecl {
        name: String,
        variants: Vec<EnumVariant>,
        attributes: Vec<Attribute>,
    },
    ModDecl {
        name: String,
        is_pub: bool,
        body: Option<Vec<Item>>,
        resolved: Option<Box<Module>>,
        attributes: Vec<Attribute>,
    },
    UseDecl {
        is_pub: bool,
        trees: Vec<UseTree>,
    },
    BlockAttribute {
        attr: Attribute,
        items: Vec<Item>,
    },
    Placement {
        macro_name: String,
        argument: PlacementArg,
        attributes: Vec<Attribute>,
    },
}

// --- Use trees --------------------------------------------------------------

/// A single import tree in a `use` declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UseTree {
    /// A path import, glob, or group.
    Path {
        root: UseRoot,
        segments: Vec<String>,
        tail: UseTail,
    },
    /// A path import with an `as alias`.
    Alias { inner: Box<UseTree>, alias: String },
}

/// The root of a use path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UseRoot {
    /// `lib::` — the current lib root.
    Lib,
    /// `self::` — the current module.
    SelfMod,
    /// `super::` — the parent module.
    Super,
    /// A dependency lib name or a relative module name.
    Name(String),
}

/// The tail of a use path after the root and intermediate segments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UseTail {
    /// The last segment is an item name.
    Item,
    /// `::*` — glob import.
    Glob,
    /// `::{a, b, c}` — group import.
    Group(Vec<UseTree>),
}

// --- Attributes -------------------------------------------------------------

/// An attribute attached to an item or block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribute {
    pub path: String,
    pub args: Vec<AttrArg>,
}

/// A single argument inside an attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttrArg {
    pub name: String,
    pub value: String,
}

// --- Function body statements ----------------------------------------------

/// A statement inside a function body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FnStmt {
    Label {
        name: String,
        stmt: Box<FnStmt>,
    },
    AsmStmt {
        opcode: String,
        operands: Vec<Operand>,
    },
    IfStmt {
        branch_hint: Option<BranchHint>,
        condition: Condition,
        then_block: Vec<FnStmt>,
        else_block: Option<Vec<FnStmt>>,
    },
    WhileStmt {
        branch_hint: Option<BranchHint>,
        condition: Condition,
        body: Vec<FnStmt>,
    },
    DoWhileStmt {
        body: Vec<FnStmt>,
        branch_hint: Option<BranchHint>,
        condition: Condition,
    },
    LoopStmt {
        body: Vec<FnStmt>,
    },
    SwitchStmt {
        register: String,
        cases: Vec<SwitchCase>,
    },
    FnCall {
        name: String,
        args: Vec<Expr>,
    },
    ReturnStmt,
    VarDeclStmt {
        decl: Box<Item>,
    },
}

// --- Operands ---------------------------------------------------------------

/// An operand in an assembly statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Operand {
    Immediate {
        value: Expr,
    },
    MemoryOperand {
        mode_prefix: Option<String>,
        expr: Expr,
        index_reg: Option<String>,
        is_indirect: bool,
    },
    RegisterRef {
        name: String,
    },
    LabelRef {
        name: String,
    },
    Selector {
        path: Vec<String>,
        accesses: Vec<Access>,
    },
}

// --- Conditions and branch hints --------------------------------------------

/// A condition in an if, while, or do-while statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub modifiers: Vec<String>,
    pub keyword: String,
}

/// A branch distance hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchHint {
    Near,
    Far,
}

/// A case in a switch statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SwitchCase {
    Case { expr: Expr, body: Vec<FnStmt> },
    Default { body: Vec<FnStmt> },
}

// --- Expressions ------------------------------------------------------------

/// An expression in the Op language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Expr {
    Number {
        value: i64,
    },
    String_ {
        value: String,
    },
    Boolean {
        value: bool,
    },
    Ident {
        name: String,
    },
    BinOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    MacroCall {
        name: String,
        arg: Box<Expr>,
    },
    Selector {
        path: Vec<String>,
        accesses: Vec<Access>,
    },
    FnCall {
        name: String,
        args: Vec<Expr>,
    },
    ParenExpr {
        inner: Box<Expr>,
    },
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Or,
    Xor,
    And,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
    Inv,
    Neg,
    Pos,
}

/// An access after a path in a selector expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Access {
    /// `::ident` — module or enum access.
    ModuleAccess { name: String },
    /// `.ident` — struct field access.
    FieldAccess { name: String },
    /// `+expr` or `-expr` — offset.
    Offset { op: OffsetOp, value: Expr },
}

/// The operator for an offset access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OffsetOp {
    Add,
    Sub,
}

// --- Types ------------------------------------------------------------------

/// A type reference in a declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Type {
    /// A named type (identifier or primitive type name).
    Named { name: String },
    /// An array type `[element]` or `[element; size]`.
    Array {
        element: Box<Type>,
        size: Option<Expr>,
    },
}

// --- Struct fields and enum variants ----------------------------------------

/// A field in a struct declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub is_volatile: bool,
    pub name: String,
    pub ty: Type,
    pub array_dim: Option<Expr>,
}

/// A variant in an enum declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<Expr>,
}

// --- Init values and placement args -----------------------------------------

/// An initializer value for a variable declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum InitValue {
    /// A single expression.
    Expr { value: Expr },
    /// A brace-enclosed list `{ a, b, c }`.
    InitList { items: Vec<InitValue> },
    /// A string literal.
    String_ { value: String },
}

/// An argument to a placement macro (`locate_bytes!`, `locate_str!`,
/// `locate_fn!`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PlacementArg {
    /// A string literal argument (for `locate_bytes!` and `locate_str!`).
    String_ { value: String },
    /// A module path argument (for `locate_fn!`).
    Path { segments: Vec<String> },
}

// --- Envelope impl ----------------------------------------------------------

impl Envelope for AstFile {
    fn version(&self) -> u32 {
        self.version
    }
}
