//! The `op-target` crate.
//!
//! This crate defines the [`Target`] trait that every CPU and platform lib
//! implements. The compiler, the parser, and the linker query the target
//! through this trait. No binary hard-codes a target.
//!
//! The crate also provides a [`Registry`] that maps a target triplet string
//! to a [`Target`] constructor. The registry loads libs from `~/.carts/` at
//! build time as described in the technical design section "Target
//! abstraction".

pub mod memory;
pub mod registry;

use op_common::TargetTriplet;

pub use memory::{MemoryKind, MemoryRegion};
pub use registry::Registry;

/// An output format for the final ROM image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    Ines,
    Lnx,
    Raw,
    Hex,
}

/// A CPU family descriptor.
pub trait Cpu: Send + Sync {
    fn opcodes(&self) -> &[OpcodeDef];
    fn registers(&self) -> &[RegisterDef];
    fn conditions(&self) -> &[ConditionDef];
    fn addressing_modes(&self) -> &[AddressingModeDef];
    fn interrupt_vectors(&self) -> &[InterruptVectorDef];
}

/// A platform descriptor.
pub trait Platform: Send + Sync {
    fn name(&self) -> &str;
    fn defines(&self) -> &[(String, u32)];
}

/// A target descriptor.
pub trait Target: Send + Sync {
    fn triplet(&self) -> &str;
    fn cpu(&self) -> &dyn Cpu;
    fn platform(&self) -> &dyn Platform;
    fn memory_map(&self) -> &[MemoryRegion];
    fn output_format(&self) -> OutputFormat;
}

/// An opcode definition from a CPU lib.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcodeDef {
    pub mnemonic: &'static str,
    pub modes: &'static [&'static str],
}

/// A register definition from a CPU lib.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterDef {
    pub name: &'static str,
    pub size: u8,
}

/// A condition keyword definition from a CPU lib.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionDef {
    pub keyword: &'static str,
    pub flag: &'static str,
    pub branch_opcode: &'static str,
}

/// An addressing mode definition from a CPU lib.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressingModeDef {
    pub name: &'static str,
}

/// An interrupt vector definition from a CPU lib.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptVectorDef {
    pub name: &'static str,
    pub address: u32,
}

/// Look up a [`Target`] by its triplet string.
pub fn lookup(registry: &Registry, triplet: &str) -> Option<std::sync::Arc<dyn Target>> {
    registry.get(triplet)
}

/// Parse a triplet string into a [`TargetTriplet`].
pub fn parse_triplet(triplet: &str) -> Result<TargetTriplet, op_common::TripletError> {
    TargetTriplet::parse(triplet)
}
