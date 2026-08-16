//! Memory map region types.
//!
//! A target's memory map is a list of [`MemoryRegion`] values. Each region
//! has a name, a kind, a base address, a size, and a bank count.

use serde::{Deserialize, Serialize};

/// The kind of a memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryKind {
    Rom,
    Ram,
    Chr,
}

/// A region in a target memory map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRegion {
    pub name: String,
    pub kind: MemoryKind,
    pub base: u32,
    pub size: u32,
    pub banks: u32,
}
