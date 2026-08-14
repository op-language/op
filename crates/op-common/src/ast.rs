//! AST node types for the parser stage.
//!
//! The parser emits the [`Module`] described in the technical design section
//! "Stage 2: parser". The node kinds match the normalized LR(1) grammar in the
//! language specification. Only the structural skeleton is defined here; the
//! parser fills in the fields as it builds the tree.

use serde::{Deserialize, Serialize};

use crate::envelope::Envelope;

/// The `.opa` post-parser AST envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstFile {
    pub version: u32,
    pub target: String,
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

/// A top-level item inside a module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Item {
    ConstDecl {
        name: String,
    },
    VarDecl {
        name: String,
    },
    FnDecl {
        name: String,
        is_noreturn: bool,
        attributes: Vec<Attribute>,
    },
    InlineFnDecl {
        name: String,
        params: Vec<String>,
    },
    StructDecl {
        name: String,
    },
    TypeDecl {
        name: String,
    },
    EnumDecl {
        name: String,
    },
    ModDecl {
        name: String,
    },
    UseDecl {
        paths: Vec<String>,
    },
}

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

impl Envelope for AstFile {
    fn version(&self) -> u32 {
        self.version
    }
}