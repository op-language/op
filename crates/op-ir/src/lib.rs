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
    /// Interrupt vector entries recorded by the codegen for the linker.
    #[serde(default)]
    pub interrupt_vectors: Vec<InterruptVector>,
    /// Header fields from #[ines(...)] or #[lnx(...)] attributes.
    #[serde(default)]
    pub header: Option<HeaderFields>,
    /// The padding byte from #[setpad(value)] or 0x00.
    #[serde(default = "default_pad_byte")]
    pub pad_byte: u8,
}

fn default_pad_byte() -> u8 {
    0x00
}

impl ObjectFile {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            version: 1,
            target: target.into(),
            sections: Vec::new(),
            interrupt_vectors: Vec::new(),
            header: None,
            pad_byte: 0x00,
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
    #[serde(default)]
    pub is_pub: bool,
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
    /// A constant added to the symbol's address when patching.
    #[serde(default)]
    pub addend: i64,
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

// --- Interrupt vectors and header metadata ----------------------------------

/// An interrupt vector entry recorded by the codegen.
///
/// The linker writes the target function address into the vector table
/// at the specified address. For the 6502, the vector addresses are
/// `reset` at `0xFFFC`, `nmi` at `0xFFFA`, and `irq` at `0xFFFE`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptVector {
    /// The interrupt name: "reset", "nmi", or "irq".
    pub name: String,
    /// The vector table address where the linker writes the 2-byte target.
    pub address: u32,
    /// The symbol name of the target function.
    pub target: String,
}

/// Header fields from `#[ines(...)]` or `#[lnx(...)]` attributes.
///
/// The file output stage reads these fields to write the output file
/// header (iNES, .lnx, or other format).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderFields {
    /// The format name: "ines", "lnx", "sega", "snes", "gb", "sms", "a78".
    pub format: String,
    /// The key-value pairs from the attribute arguments.
    pub fields: Vec<(String, String)>,
}
