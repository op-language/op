//! Intermediate representation types for the Op compiler and linker.
//!
//! This crate defines the section, symbol, relocation, and linked-data types
//! that the code generator, the optimizer, and the linker produce. The
//! `.opl` intermediate file format (post-compile and post-link) serializes
//! these types to JSON as described in the technical design section
//! "Intermediate file formats".

use serde::{Deserialize, Serialize};

use op_common::Envelope;

/// The `.opl` post-compile or post-link object data envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectFile {
    pub version: u32,
    pub target: String,
    pub sections: Vec<Section>,
}

impl ObjectFile {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            version: 1,
            target: target.into(),
            sections: Vec::new(),
        }
    }
}

impl Envelope for ObjectFile {
    fn version(&self) -> u32 {
        self.version
    }
}

/// A section of object data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub kind: SectionKind,
    pub org: u32,
    pub bank: u32,
    pub maxsize: u32,
    pub symbols: Vec<Symbol>,
    pub relocations: Vec<Relocation>,
    pub data: Vec<u8>,
}

/// The kind of a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SectionKind {
    Rom,
    Ram,
    Chr,
}

/// A symbol inside a section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub offset: u32,
    pub size: u32,
    pub kind: SymbolKind,
}

/// The kind of a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Variable,
    Label,
}

/// A relocation entry that the linker patches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relocation {
    pub offset: u32,
    pub kind: RelocKind,
    pub symbol: String,
}

/// The relocation kinds defined in the technical design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RelocKind {
    Abs8,
    Abs16,
    Abs24,
    Abs32,
    Branch8,
    Branch16,
    Lo8,
    Hi8,
    Bank,
}