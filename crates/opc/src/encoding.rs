//! CPU opcode encoding tables.
//!
//! This module defines a static encoding table for each CPU family. Each
//! table maps a (mnemonic, addressing_mode) pair to an opcode byte. The
//! codegen uses these tables to encode assembly statements into bytes.

/// An addressing mode identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddrMode {
    Implied,
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    Relative,
}

/// An encoding table entry: (mnemonic, addressing_mode, opcode_byte).
pub struct EncodingEntry {
    pub mnemonic: &'static str,
    pub mode: AddrMode,
    pub opcode: u8,
}

/// Look up the opcode byte for a given mnemonic and addressing mode in
/// a CPU family's encoding table.
pub fn lookup_opcode(table: &[EncodingEntry], mnemonic: &str, mode: AddrMode) -> Option<u8> {
    table
        .iter()
        .find(|e| e.mnemonic.eq_ignore_ascii_case(mnemonic) && e.mode == mode)
        .map(|e| e.opcode)
}

/// Look up the opcode byte in a vector of encoding entries (used by the
/// codegen which holds a combined table).
pub fn lookup_opcode_in(table: &[&EncodingEntry], mnemonic: &str, mode: AddrMode) -> Option<u8> {
    table
        .iter()
        .find(|e| e.mnemonic.eq_ignore_ascii_case(mnemonic) && e.mode == mode)
        .map(|e| e.opcode)
}

// --- 6502 encoding table ----------------------------------------------------

pub const ENCODING_6502: &[EncodingEntry] = &[
    // ADC
    EncodingEntry {
        mnemonic: "adc",
        mode: AddrMode::Immediate,
        opcode: 0x69,
    },
    EncodingEntry {
        mnemonic: "adc",
        mode: AddrMode::ZeroPage,
        opcode: 0x65,
    },
    EncodingEntry {
        mnemonic: "adc",
        mode: AddrMode::ZeroPageX,
        opcode: 0x75,
    },
    EncodingEntry {
        mnemonic: "adc",
        mode: AddrMode::Absolute,
        opcode: 0x6D,
    },
    EncodingEntry {
        mnemonic: "adc",
        mode: AddrMode::AbsoluteX,
        opcode: 0x7D,
    },
    EncodingEntry {
        mnemonic: "adc",
        mode: AddrMode::AbsoluteY,
        opcode: 0x79,
    },
    EncodingEntry {
        mnemonic: "adc",
        mode: AddrMode::IndirectY,
        opcode: 0x71,
    },
    EncodingEntry {
        mnemonic: "adc",
        mode: AddrMode::IndirectX,
        opcode: 0x61,
    },
    // AND
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::Immediate,
        opcode: 0x29,
    },
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::ZeroPage,
        opcode: 0x25,
    },
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::ZeroPageX,
        opcode: 0x35,
    },
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::Absolute,
        opcode: 0x2D,
    },
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::AbsoluteX,
        opcode: 0x3D,
    },
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::AbsoluteY,
        opcode: 0x39,
    },
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::IndirectY,
        opcode: 0x31,
    },
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::IndirectX,
        opcode: 0x21,
    },
    // ASL
    EncodingEntry {
        mnemonic: "asl",
        mode: AddrMode::Accumulator,
        opcode: 0x0A,
    },
    EncodingEntry {
        mnemonic: "asl",
        mode: AddrMode::ZeroPage,
        opcode: 0x06,
    },
    EncodingEntry {
        mnemonic: "asl",
        mode: AddrMode::ZeroPageX,
        opcode: 0x16,
    },
    EncodingEntry {
        mnemonic: "asl",
        mode: AddrMode::Absolute,
        opcode: 0x0E,
    },
    EncodingEntry {
        mnemonic: "asl",
        mode: AddrMode::AbsoluteX,
        opcode: 0x1E,
    },
    // BCC/BCS/BEQ/BMI/BNE/BPL/BVC/BVS
    EncodingEntry {
        mnemonic: "bcc",
        mode: AddrMode::Relative,
        opcode: 0x90,
    },
    EncodingEntry {
        mnemonic: "bcs",
        mode: AddrMode::Relative,
        opcode: 0xB0,
    },
    EncodingEntry {
        mnemonic: "beq",
        mode: AddrMode::Relative,
        opcode: 0xF0,
    },
    EncodingEntry {
        mnemonic: "bmi",
        mode: AddrMode::Relative,
        opcode: 0x30,
    },
    EncodingEntry {
        mnemonic: "bne",
        mode: AddrMode::Relative,
        opcode: 0xD0,
    },
    EncodingEntry {
        mnemonic: "bpl",
        mode: AddrMode::Relative,
        opcode: 0x10,
    },
    EncodingEntry {
        mnemonic: "bvc",
        mode: AddrMode::Relative,
        opcode: 0x50,
    },
    EncodingEntry {
        mnemonic: "bvs",
        mode: AddrMode::Relative,
        opcode: 0x70,
    },
    // BIT
    EncodingEntry {
        mnemonic: "bit",
        mode: AddrMode::ZeroPage,
        opcode: 0x24,
    },
    EncodingEntry {
        mnemonic: "bit",
        mode: AddrMode::Absolute,
        opcode: 0x2C,
    },
    // BRK
    EncodingEntry {
        mnemonic: "brk",
        mode: AddrMode::Implied,
        opcode: 0x00,
    },
    // CLC/CLD/CLI/CLV
    EncodingEntry {
        mnemonic: "clc",
        mode: AddrMode::Implied,
        opcode: 0x18,
    },
    EncodingEntry {
        mnemonic: "cld",
        mode: AddrMode::Implied,
        opcode: 0xD8,
    },
    EncodingEntry {
        mnemonic: "cli",
        mode: AddrMode::Implied,
        opcode: 0x58,
    },
    EncodingEntry {
        mnemonic: "clv",
        mode: AddrMode::Implied,
        opcode: 0xB8,
    },
    // CMP
    EncodingEntry {
        mnemonic: "cmp",
        mode: AddrMode::Immediate,
        opcode: 0xC9,
    },
    EncodingEntry {
        mnemonic: "cmp",
        mode: AddrMode::ZeroPage,
        opcode: 0xC5,
    },
    EncodingEntry {
        mnemonic: "cmp",
        mode: AddrMode::ZeroPageX,
        opcode: 0xD5,
    },
    EncodingEntry {
        mnemonic: "cmp",
        mode: AddrMode::Absolute,
        opcode: 0xCD,
    },
    EncodingEntry {
        mnemonic: "cmp",
        mode: AddrMode::AbsoluteX,
        opcode: 0xDD,
    },
    EncodingEntry {
        mnemonic: "cmp",
        mode: AddrMode::AbsoluteY,
        opcode: 0xD9,
    },
    EncodingEntry {
        mnemonic: "cmp",
        mode: AddrMode::IndirectY,
        opcode: 0xD1,
    },
    EncodingEntry {
        mnemonic: "cmp",
        mode: AddrMode::IndirectX,
        opcode: 0xC1,
    },
    // CPX/CPY
    EncodingEntry {
        mnemonic: "cpx",
        mode: AddrMode::Immediate,
        opcode: 0xE0,
    },
    EncodingEntry {
        mnemonic: "cpx",
        mode: AddrMode::ZeroPage,
        opcode: 0xE4,
    },
    EncodingEntry {
        mnemonic: "cpx",
        mode: AddrMode::Absolute,
        opcode: 0xEC,
    },
    EncodingEntry {
        mnemonic: "cpy",
        mode: AddrMode::Immediate,
        opcode: 0xC0,
    },
    EncodingEntry {
        mnemonic: "cpy",
        mode: AddrMode::ZeroPage,
        opcode: 0xC4,
    },
    EncodingEntry {
        mnemonic: "cpy",
        mode: AddrMode::Absolute,
        opcode: 0xCC,
    },
    // DEC
    EncodingEntry {
        mnemonic: "dec",
        mode: AddrMode::ZeroPage,
        opcode: 0xC6,
    },
    EncodingEntry {
        mnemonic: "dec",
        mode: AddrMode::ZeroPageX,
        opcode: 0xD6,
    },
    EncodingEntry {
        mnemonic: "dec",
        mode: AddrMode::Absolute,
        opcode: 0xCE,
    },
    EncodingEntry {
        mnemonic: "dec",
        mode: AddrMode::AbsoluteX,
        opcode: 0xDE,
    },
    // DEX/DEY
    EncodingEntry {
        mnemonic: "dex",
        mode: AddrMode::Implied,
        opcode: 0xCA,
    },
    EncodingEntry {
        mnemonic: "dey",
        mode: AddrMode::Implied,
        opcode: 0x88,
    },
    // EOR
    EncodingEntry {
        mnemonic: "eor",
        mode: AddrMode::Immediate,
        opcode: 0x49,
    },
    EncodingEntry {
        mnemonic: "eor",
        mode: AddrMode::ZeroPage,
        opcode: 0x45,
    },
    EncodingEntry {
        mnemonic: "eor",
        mode: AddrMode::ZeroPageX,
        opcode: 0x55,
    },
    EncodingEntry {
        mnemonic: "eor",
        mode: AddrMode::Absolute,
        opcode: 0x4D,
    },
    EncodingEntry {
        mnemonic: "eor",
        mode: AddrMode::AbsoluteX,
        opcode: 0x5D,
    },
    EncodingEntry {
        mnemonic: "eor",
        mode: AddrMode::AbsoluteY,
        opcode: 0x59,
    },
    EncodingEntry {
        mnemonic: "eor",
        mode: AddrMode::IndirectY,
        opcode: 0x51,
    },
    EncodingEntry {
        mnemonic: "eor",
        mode: AddrMode::IndirectX,
        opcode: 0x41,
    },
    // INC
    EncodingEntry {
        mnemonic: "inc",
        mode: AddrMode::ZeroPage,
        opcode: 0xE6,
    },
    EncodingEntry {
        mnemonic: "inc",
        mode: AddrMode::ZeroPageX,
        opcode: 0xF6,
    },
    EncodingEntry {
        mnemonic: "inc",
        mode: AddrMode::Absolute,
        opcode: 0xEE,
    },
    EncodingEntry {
        mnemonic: "inc",
        mode: AddrMode::AbsoluteX,
        opcode: 0xFE,
    },
    // INX/INY
    EncodingEntry {
        mnemonic: "inx",
        mode: AddrMode::Implied,
        opcode: 0xE8,
    },
    EncodingEntry {
        mnemonic: "iny",
        mode: AddrMode::Implied,
        opcode: 0xC8,
    },
    // JMP
    EncodingEntry {
        mnemonic: "jmp",
        mode: AddrMode::Absolute,
        opcode: 0x4C,
    },
    EncodingEntry {
        mnemonic: "jmp",
        mode: AddrMode::Indirect,
        opcode: 0x6C,
    },
    // JSR
    EncodingEntry {
        mnemonic: "jsr",
        mode: AddrMode::Absolute,
        opcode: 0x20,
    },
    // LDA
    EncodingEntry {
        mnemonic: "lda",
        mode: AddrMode::Immediate,
        opcode: 0xA9,
    },
    EncodingEntry {
        mnemonic: "lda",
        mode: AddrMode::ZeroPage,
        opcode: 0xA5,
    },
    EncodingEntry {
        mnemonic: "lda",
        mode: AddrMode::ZeroPageX,
        opcode: 0xB5,
    },
    EncodingEntry {
        mnemonic: "lda",
        mode: AddrMode::Absolute,
        opcode: 0xAD,
    },
    EncodingEntry {
        mnemonic: "lda",
        mode: AddrMode::AbsoluteX,
        opcode: 0xBD,
    },
    EncodingEntry {
        mnemonic: "lda",
        mode: AddrMode::AbsoluteY,
        opcode: 0xB9,
    },
    EncodingEntry {
        mnemonic: "lda",
        mode: AddrMode::IndirectY,
        opcode: 0xB1,
    },
    EncodingEntry {
        mnemonic: "lda",
        mode: AddrMode::IndirectX,
        opcode: 0xA1,
    },
    // LDX
    EncodingEntry {
        mnemonic: "ldx",
        mode: AddrMode::Immediate,
        opcode: 0xA2,
    },
    EncodingEntry {
        mnemonic: "ldx",
        mode: AddrMode::ZeroPage,
        opcode: 0xA6,
    },
    EncodingEntry {
        mnemonic: "ldx",
        mode: AddrMode::ZeroPageY,
        opcode: 0xB6,
    },
    EncodingEntry {
        mnemonic: "ldx",
        mode: AddrMode::Absolute,
        opcode: 0xAE,
    },
    EncodingEntry {
        mnemonic: "ldx",
        mode: AddrMode::AbsoluteY,
        opcode: 0xBE,
    },
    // LDY
    EncodingEntry {
        mnemonic: "ldy",
        mode: AddrMode::Immediate,
        opcode: 0xA0,
    },
    EncodingEntry {
        mnemonic: "ldy",
        mode: AddrMode::ZeroPage,
        opcode: 0xA4,
    },
    EncodingEntry {
        mnemonic: "ldy",
        mode: AddrMode::ZeroPageX,
        opcode: 0xB4,
    },
    EncodingEntry {
        mnemonic: "ldy",
        mode: AddrMode::Absolute,
        opcode: 0xAC,
    },
    EncodingEntry {
        mnemonic: "ldy",
        mode: AddrMode::AbsoluteX,
        opcode: 0xBC,
    },
    // LSR
    EncodingEntry {
        mnemonic: "lsr",
        mode: AddrMode::Accumulator,
        opcode: 0x4A,
    },
    EncodingEntry {
        mnemonic: "lsr",
        mode: AddrMode::ZeroPage,
        opcode: 0x46,
    },
    EncodingEntry {
        mnemonic: "lsr",
        mode: AddrMode::ZeroPageX,
        opcode: 0x56,
    },
    EncodingEntry {
        mnemonic: "lsr",
        mode: AddrMode::Absolute,
        opcode: 0x4E,
    },
    EncodingEntry {
        mnemonic: "lsr",
        mode: AddrMode::AbsoluteX,
        opcode: 0x5E,
    },
    // NOP
    EncodingEntry {
        mnemonic: "nop",
        mode: AddrMode::Implied,
        opcode: 0xEA,
    },
    // ORA
    EncodingEntry {
        mnemonic: "ora",
        mode: AddrMode::Immediate,
        opcode: 0x09,
    },
    EncodingEntry {
        mnemonic: "ora",
        mode: AddrMode::ZeroPage,
        opcode: 0x05,
    },
    EncodingEntry {
        mnemonic: "ora",
        mode: AddrMode::ZeroPageX,
        opcode: 0x15,
    },
    EncodingEntry {
        mnemonic: "ora",
        mode: AddrMode::Absolute,
        opcode: 0x0D,
    },
    EncodingEntry {
        mnemonic: "ora",
        mode: AddrMode::AbsoluteX,
        opcode: 0x1D,
    },
    EncodingEntry {
        mnemonic: "ora",
        mode: AddrMode::AbsoluteY,
        opcode: 0x19,
    },
    EncodingEntry {
        mnemonic: "ora",
        mode: AddrMode::IndirectY,
        opcode: 0x11,
    },
    EncodingEntry {
        mnemonic: "ora",
        mode: AddrMode::IndirectX,
        opcode: 0x01,
    },
    // PHA/PHP/PLA/PLP
    EncodingEntry {
        mnemonic: "pha",
        mode: AddrMode::Implied,
        opcode: 0x48,
    },
    EncodingEntry {
        mnemonic: "php",
        mode: AddrMode::Implied,
        opcode: 0x08,
    },
    EncodingEntry {
        mnemonic: "pla",
        mode: AddrMode::Implied,
        opcode: 0x68,
    },
    EncodingEntry {
        mnemonic: "plp",
        mode: AddrMode::Implied,
        opcode: 0x28,
    },
    // ROL
    EncodingEntry {
        mnemonic: "rol",
        mode: AddrMode::Accumulator,
        opcode: 0x2A,
    },
    EncodingEntry {
        mnemonic: "rol",
        mode: AddrMode::ZeroPage,
        opcode: 0x26,
    },
    EncodingEntry {
        mnemonic: "rol",
        mode: AddrMode::ZeroPageX,
        opcode: 0x36,
    },
    EncodingEntry {
        mnemonic: "rol",
        mode: AddrMode::Absolute,
        opcode: 0x2E,
    },
    EncodingEntry {
        mnemonic: "rol",
        mode: AddrMode::AbsoluteX,
        opcode: 0x3E,
    },
    // ROR
    EncodingEntry {
        mnemonic: "ror",
        mode: AddrMode::Accumulator,
        opcode: 0x6A,
    },
    EncodingEntry {
        mnemonic: "ror",
        mode: AddrMode::ZeroPage,
        opcode: 0x66,
    },
    EncodingEntry {
        mnemonic: "ror",
        mode: AddrMode::ZeroPageX,
        opcode: 0x76,
    },
    EncodingEntry {
        mnemonic: "ror",
        mode: AddrMode::Absolute,
        opcode: 0x6E,
    },
    EncodingEntry {
        mnemonic: "ror",
        mode: AddrMode::AbsoluteX,
        opcode: 0x7E,
    },
    // RTI/RTS
    EncodingEntry {
        mnemonic: "rti",
        mode: AddrMode::Implied,
        opcode: 0x40,
    },
    EncodingEntry {
        mnemonic: "rts",
        mode: AddrMode::Implied,
        opcode: 0x60,
    },
    // SBC
    EncodingEntry {
        mnemonic: "sbc",
        mode: AddrMode::Immediate,
        opcode: 0xE9,
    },
    EncodingEntry {
        mnemonic: "sbc",
        mode: AddrMode::ZeroPage,
        opcode: 0xE5,
    },
    EncodingEntry {
        mnemonic: "sbc",
        mode: AddrMode::ZeroPageX,
        opcode: 0xF5,
    },
    EncodingEntry {
        mnemonic: "sbc",
        mode: AddrMode::Absolute,
        opcode: 0xED,
    },
    EncodingEntry {
        mnemonic: "sbc",
        mode: AddrMode::AbsoluteX,
        opcode: 0xFD,
    },
    EncodingEntry {
        mnemonic: "sbc",
        mode: AddrMode::AbsoluteY,
        opcode: 0xF9,
    },
    EncodingEntry {
        mnemonic: "sbc",
        mode: AddrMode::IndirectY,
        opcode: 0xF1,
    },
    EncodingEntry {
        mnemonic: "sbc",
        mode: AddrMode::IndirectX,
        opcode: 0xE1,
    },
    // SEC/SED/SEI
    EncodingEntry {
        mnemonic: "sec",
        mode: AddrMode::Implied,
        opcode: 0x38,
    },
    EncodingEntry {
        mnemonic: "sed",
        mode: AddrMode::Implied,
        opcode: 0xF8,
    },
    EncodingEntry {
        mnemonic: "sei",
        mode: AddrMode::Implied,
        opcode: 0x78,
    },
    // STA
    EncodingEntry {
        mnemonic: "sta",
        mode: AddrMode::ZeroPage,
        opcode: 0x85,
    },
    EncodingEntry {
        mnemonic: "sta",
        mode: AddrMode::ZeroPageX,
        opcode: 0x95,
    },
    EncodingEntry {
        mnemonic: "sta",
        mode: AddrMode::Absolute,
        opcode: 0x8D,
    },
    EncodingEntry {
        mnemonic: "sta",
        mode: AddrMode::AbsoluteX,
        opcode: 0x9D,
    },
    EncodingEntry {
        mnemonic: "sta",
        mode: AddrMode::AbsoluteY,
        opcode: 0x99,
    },
    EncodingEntry {
        mnemonic: "sta",
        mode: AddrMode::IndirectY,
        opcode: 0x91,
    },
    EncodingEntry {
        mnemonic: "sta",
        mode: AddrMode::IndirectX,
        opcode: 0x81,
    },
    // STX
    EncodingEntry {
        mnemonic: "stx",
        mode: AddrMode::ZeroPage,
        opcode: 0x86,
    },
    EncodingEntry {
        mnemonic: "stx",
        mode: AddrMode::ZeroPageY,
        opcode: 0x96,
    },
    EncodingEntry {
        mnemonic: "stx",
        mode: AddrMode::Absolute,
        opcode: 0x8E,
    },
    // STY
    EncodingEntry {
        mnemonic: "sty",
        mode: AddrMode::ZeroPage,
        opcode: 0x84,
    },
    EncodingEntry {
        mnemonic: "sty",
        mode: AddrMode::ZeroPageX,
        opcode: 0x94,
    },
    EncodingEntry {
        mnemonic: "sty",
        mode: AddrMode::Absolute,
        opcode: 0x8C,
    },
    // TAX/TAY/TSX/TXA/TXS/TYA
    EncodingEntry {
        mnemonic: "tax",
        mode: AddrMode::Implied,
        opcode: 0xAA,
    },
    EncodingEntry {
        mnemonic: "tay",
        mode: AddrMode::Implied,
        opcode: 0xA8,
    },
    EncodingEntry {
        mnemonic: "tsx",
        mode: AddrMode::Implied,
        opcode: 0xBA,
    },
    EncodingEntry {
        mnemonic: "txa",
        mode: AddrMode::Implied,
        opcode: 0x8A,
    },
    EncodingEntry {
        mnemonic: "txs",
        mode: AddrMode::Implied,
        opcode: 0x9A,
    },
    EncodingEntry {
        mnemonic: "tya",
        mode: AddrMode::Implied,
        opcode: 0x98,
    },
];

// --- 65SC02 additional opcodes ---------------------------------------------

pub const ENCODING_65SC02: &[EncodingEntry] = &[
    // BRA
    EncodingEntry {
        mnemonic: "bra",
        mode: AddrMode::Relative,
        opcode: 0x80,
    },
    // PHX/PHY/PLX/PLY
    EncodingEntry {
        mnemonic: "phx",
        mode: AddrMode::Implied,
        opcode: 0xDA,
    },
    EncodingEntry {
        mnemonic: "phy",
        mode: AddrMode::Implied,
        opcode: 0x5A,
    },
    EncodingEntry {
        mnemonic: "plx",
        mode: AddrMode::Implied,
        opcode: 0xFA,
    },
    EncodingEntry {
        mnemonic: "ply",
        mode: AddrMode::Implied,
        opcode: 0x7A,
    },
    // STZ
    EncodingEntry {
        mnemonic: "stz",
        mode: AddrMode::ZeroPage,
        opcode: 0x64,
    },
    EncodingEntry {
        mnemonic: "stz",
        mode: AddrMode::ZeroPageX,
        opcode: 0x74,
    },
    EncodingEntry {
        mnemonic: "stz",
        mode: AddrMode::Absolute,
        opcode: 0x9C,
    },
    EncodingEntry {
        mnemonic: "stz",
        mode: AddrMode::AbsoluteX,
        opcode: 0x9E,
    },
    // TSB/TRB
    EncodingEntry {
        mnemonic: "tsb",
        mode: AddrMode::ZeroPage,
        opcode: 0x04,
    },
    EncodingEntry {
        mnemonic: "tsb",
        mode: AddrMode::Absolute,
        opcode: 0x0C,
    },
    EncodingEntry {
        mnemonic: "trb",
        mode: AddrMode::ZeroPage,
        opcode: 0x14,
    },
    EncodingEntry {
        mnemonic: "trb",
        mode: AddrMode::Absolute,
        opcode: 0x1C,
    },
    // INA/DEA
    EncodingEntry {
        mnemonic: "ina",
        mode: AddrMode::Implied,
        opcode: 0x1A,
    },
    EncodingEntry {
        mnemonic: "dea",
        mode: AddrMode::Implied,
        opcode: 0x3A,
    },
    // Additional addressing modes for existing opcodes
    EncodingEntry {
        mnemonic: "adc",
        mode: AddrMode::Indirect,
        opcode: 0x72,
    },
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::Indirect,
        opcode: 0x32,
    },
    EncodingEntry {
        mnemonic: "cmp",
        mode: AddrMode::Indirect,
        opcode: 0xD2,
    },
    EncodingEntry {
        mnemonic: "eor",
        mode: AddrMode::Indirect,
        opcode: 0x52,
    },
    EncodingEntry {
        mnemonic: "ora",
        mode: AddrMode::Indirect,
        opcode: 0x12,
    },
    EncodingEntry {
        mnemonic: "sbc",
        mode: AddrMode::Indirect,
        opcode: 0xF2,
    },
    EncodingEntry {
        mnemonic: "bit",
        mode: AddrMode::Immediate,
        opcode: 0x89,
    },
    EncodingEntry {
        mnemonic: "bit",
        mode: AddrMode::ZeroPageX,
        opcode: 0x34,
    },
    EncodingEntry {
        mnemonic: "bit",
        mode: AddrMode::AbsoluteX,
        opcode: 0x3C,
    },
];

// --- 65C816 additional opcodes ---------------------------------------------

pub const ENCODING_65C816: &[EncodingEntry] = &[
    EncodingEntry {
        mnemonic: "rep",
        mode: AddrMode::Immediate,
        opcode: 0xC2,
    },
    EncodingEntry {
        mnemonic: "sep",
        mode: AddrMode::Immediate,
        opcode: 0xE2,
    },
    EncodingEntry {
        mnemonic: "xba",
        mode: AddrMode::Implied,
        opcode: 0xEB,
    },
    EncodingEntry {
        mnemonic: "xce",
        mode: AddrMode::Implied,
        opcode: 0xFB,
    },
    EncodingEntry {
        mnemonic: "tcd",
        mode: AddrMode::Implied,
        opcode: 0x5B,
    },
    EncodingEntry {
        mnemonic: "tdc",
        mode: AddrMode::Implied,
        opcode: 0x7B,
    },
    EncodingEntry {
        mnemonic: "tcs",
        mode: AddrMode::Implied,
        opcode: 0x1B,
    },
    EncodingEntry {
        mnemonic: "tsc",
        mode: AddrMode::Implied,
        opcode: 0x3B,
    },
    EncodingEntry {
        mnemonic: "txy",
        mode: AddrMode::Implied,
        opcode: 0x9B,
    },
    EncodingEntry {
        mnemonic: "tyx",
        mode: AddrMode::Implied,
        opcode: 0xBB,
    },
    EncodingEntry {
        mnemonic: "mvn",
        mode: AddrMode::Implied,
        opcode: 0x54,
    },
    EncodingEntry {
        mnemonic: "mvp",
        mode: AddrMode::Implied,
        opcode: 0x44,
    },
    EncodingEntry {
        mnemonic: "pea",
        mode: AddrMode::Absolute,
        opcode: 0xF4,
    },
    EncodingEntry {
        mnemonic: "pei",
        mode: AddrMode::Indirect,
        opcode: 0xD4,
    },
    EncodingEntry {
        mnemonic: "per",
        mode: AddrMode::Absolute,
        opcode: 0x62,
    },
    EncodingEntry {
        mnemonic: "jml",
        mode: AddrMode::Absolute,
        opcode: 0x5C,
    },
    EncodingEntry {
        mnemonic: "jsl",
        mode: AddrMode::Absolute,
        opcode: 0x22,
    },
    EncodingEntry {
        mnemonic: "rtl",
        mode: AddrMode::Implied,
        opcode: 0x6B,
    },
    EncodingEntry {
        mnemonic: "cop",
        mode: AddrMode::Implied,
        opcode: 0x02,
    },
    EncodingEntry {
        mnemonic: "wai",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    EncodingEntry {
        mnemonic: "stp",
        mode: AddrMode::Implied,
        opcode: 0xDB,
    },
];

// --- Z80 encoding table ----------------------------------------------------

pub const ENCODING_Z80: &[EncodingEntry] = &[
    // LD r,n — immediate loads (using a simplified model: LD A,n = 0x3E)
    EncodingEntry {
        mnemonic: "ld",
        mode: AddrMode::Immediate,
        opcode: 0x3E,
    },
    EncodingEntry {
        mnemonic: "ld",
        mode: AddrMode::Absolute,
        opcode: 0x32,
    },
    // PUSH/POP
    EncodingEntry {
        mnemonic: "push",
        mode: AddrMode::Implied,
        opcode: 0xC5,
    },
    EncodingEntry {
        mnemonic: "pop",
        mode: AddrMode::Implied,
        opcode: 0xC1,
    },
    // EX/EXX
    EncodingEntry {
        mnemonic: "ex",
        mode: AddrMode::Implied,
        opcode: 0x08,
    },
    EncodingEntry {
        mnemonic: "exx",
        mode: AddrMode::Implied,
        opcode: 0xD9,
    },
    // LDI/LDIR/LDD/LDDR
    EncodingEntry {
        mnemonic: "ldi",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    EncodingEntry {
        mnemonic: "ldir",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    EncodingEntry {
        mnemonic: "ldd",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    EncodingEntry {
        mnemonic: "lddr",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    // CPI/CPIR/CPD/CPDR
    EncodingEntry {
        mnemonic: "cpi",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    EncodingEntry {
        mnemonic: "cpir",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    EncodingEntry {
        mnemonic: "cpd",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    EncodingEntry {
        mnemonic: "cpdr",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    // ADC/SBC
    EncodingEntry {
        mnemonic: "adc",
        mode: AddrMode::Immediate,
        opcode: 0xCE,
    },
    EncodingEntry {
        mnemonic: "sbc",
        mode: AddrMode::Immediate,
        opcode: 0xDE,
    },
    // CP
    EncodingEntry {
        mnemonic: "cp",
        mode: AddrMode::Immediate,
        opcode: 0xFE,
    },
    // INC/DEC
    EncodingEntry {
        mnemonic: "inc",
        mode: AddrMode::Implied,
        opcode: 0x3C,
    },
    EncodingEntry {
        mnemonic: "dec",
        mode: AddrMode::Implied,
        opcode: 0x3D,
    },
    // DAA/CPL/NEG/CCF/SCF
    EncodingEntry {
        mnemonic: "daa",
        mode: AddrMode::Implied,
        opcode: 0x27,
    },
    EncodingEntry {
        mnemonic: "cpl",
        mode: AddrMode::Implied,
        opcode: 0x2F,
    },
    EncodingEntry {
        mnemonic: "neg",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    EncodingEntry {
        mnemonic: "ccf",
        mode: AddrMode::Implied,
        opcode: 0x3F,
    },
    EncodingEntry {
        mnemonic: "scf",
        mode: AddrMode::Implied,
        opcode: 0x37,
    },
    // NOP/HALT/DI/EI
    EncodingEntry {
        mnemonic: "nop",
        mode: AddrMode::Implied,
        opcode: 0x00,
    },
    EncodingEntry {
        mnemonic: "halt",
        mode: AddrMode::Implied,
        opcode: 0x76,
    },
    EncodingEntry {
        mnemonic: "di",
        mode: AddrMode::Implied,
        opcode: 0xF3,
    },
    EncodingEntry {
        mnemonic: "ei",
        mode: AddrMode::Implied,
        opcode: 0xFB,
    },
    // JP/JR/DJNZ
    EncodingEntry {
        mnemonic: "jp",
        mode: AddrMode::Absolute,
        opcode: 0xC3,
    },
    EncodingEntry {
        mnemonic: "jp",
        mode: AddrMode::Immediate,
        opcode: 0xC3,
    },
    EncodingEntry {
        mnemonic: "jr",
        mode: AddrMode::Relative,
        opcode: 0x18,
    },
    EncodingEntry {
        mnemonic: "djnz",
        mode: AddrMode::Relative,
        opcode: 0x10,
    },
    // CALL/RET/RETI/RETN
    EncodingEntry {
        mnemonic: "call",
        mode: AddrMode::Absolute,
        opcode: 0xCD,
    },
    EncodingEntry {
        mnemonic: "ret",
        mode: AddrMode::Implied,
        opcode: 0xC9,
    },
    EncodingEntry {
        mnemonic: "reti",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    EncodingEntry {
        mnemonic: "retn",
        mode: AddrMode::Implied,
        opcode: 0xED,
    },
    // RST
    EncodingEntry {
        mnemonic: "rst",
        mode: AddrMode::Implied,
        opcode: 0xC7,
    },
    // ADD/AND/OR/XOR/SUB
    EncodingEntry {
        mnemonic: "add",
        mode: AddrMode::Immediate,
        opcode: 0xC6,
    },
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::Immediate,
        opcode: 0xE6,
    },
    EncodingEntry {
        mnemonic: "or",
        mode: AddrMode::Immediate,
        opcode: 0xF6,
    },
    EncodingEntry {
        mnemonic: "xor",
        mode: AddrMode::Immediate,
        opcode: 0xEE,
    },
    EncodingEntry {
        mnemonic: "sub",
        mode: AddrMode::Immediate,
        opcode: 0xD6,
    },
    // RLC/RL/RRC/RR/SLA/SRA/SLL/SRL
    EncodingEntry {
        mnemonic: "rlc",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    EncodingEntry {
        mnemonic: "rl",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    EncodingEntry {
        mnemonic: "rrc",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    EncodingEntry {
        mnemonic: "rr",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    EncodingEntry {
        mnemonic: "sla",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    EncodingEntry {
        mnemonic: "sra",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    EncodingEntry {
        mnemonic: "sll",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    EncodingEntry {
        mnemonic: "srl",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    // RLCA/RRCA/RRA
    EncodingEntry {
        mnemonic: "rlca",
        mode: AddrMode::Implied,
        opcode: 0x07,
    },
    EncodingEntry {
        mnemonic: "rrca",
        mode: AddrMode::Implied,
        opcode: 0x0F,
    },
    EncodingEntry {
        mnemonic: "rra",
        mode: AddrMode::Implied,
        opcode: 0x1F,
    },
    // IM
    EncodingEntry {
        mnemonic: "im",
        mode: AddrMode::Immediate,
        opcode: 0xED,
    },
    // IN/OUT
    EncodingEntry {
        mnemonic: "in",
        mode: AddrMode::Implied,
        opcode: 0xDB,
    },
    EncodingEntry {
        mnemonic: "out",
        mode: AddrMode::Implied,
        opcode: 0xD3,
    },
    // BIT/SET/RES
    EncodingEntry {
        mnemonic: "bit",
        mode: AddrMode::Immediate,
        opcode: 0xCB,
    },
    EncodingEntry {
        mnemonic: "set",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    EncodingEntry {
        mnemonic: "res",
        mode: AddrMode::Implied,
        opcode: 0xCB,
    },
    // STOP
    EncodingEntry {
        mnemonic: "stop",
        mode: AddrMode::Implied,
        opcode: 0x10,
    },
    // LDH (LR35902 specific, but included here for Z80 compatibility)
    EncodingEntry {
        mnemonic: "ldh",
        mode: AddrMode::Immediate,
        opcode: 0xF0,
    },
];

// --- 68000 encoding table ---------------------------------------------------
// The 68000 uses 16-bit opcode words. The encoding is simplified here —
// each mnemonic maps to a base opcode word. The actual encoding requires
// mode/reg fields that are filled in by the codegen based on the operand
// form. This table provides the base opcode for each mnemonic.

pub const ENCODING_68000: &[EncodingEntry] = &[
    EncodingEntry {
        mnemonic: "move",
        mode: AddrMode::Immediate,
        opcode: 0x00,
    },
    EncodingEntry {
        mnemonic: "move",
        mode: AddrMode::Absolute,
        opcode: 0x00,
    },
    EncodingEntry {
        mnemonic: "moveq",
        mode: AddrMode::Immediate,
        opcode: 0x70,
    },
    EncodingEntry {
        mnemonic: "lea",
        mode: AddrMode::Absolute,
        opcode: 0x41,
    },
    EncodingEntry {
        mnemonic: "clr",
        mode: AddrMode::Implied,
        opcode: 0x42,
    },
    EncodingEntry {
        mnemonic: "not",
        mode: AddrMode::Implied,
        opcode: 0x46,
    },
    EncodingEntry {
        mnemonic: "add",
        mode: AddrMode::Immediate,
        opcode: 0x06,
    },
    EncodingEntry {
        mnemonic: "addq",
        mode: AddrMode::Immediate,
        opcode: 0x50,
    },
    EncodingEntry {
        mnemonic: "sub",
        mode: AddrMode::Immediate,
        opcode: 0x04,
    },
    EncodingEntry {
        mnemonic: "subq",
        mode: AddrMode::Immediate,
        opcode: 0x51,
    },
    EncodingEntry {
        mnemonic: "mulu",
        mode: AddrMode::Implied,
        opcode: 0xC0,
    },
    EncodingEntry {
        mnemonic: "muls",
        mode: AddrMode::Implied,
        opcode: 0xC1,
    },
    EncodingEntry {
        mnemonic: "divu",
        mode: AddrMode::Implied,
        opcode: 0x80,
    },
    EncodingEntry {
        mnemonic: "divs",
        mode: AddrMode::Implied,
        opcode: 0x81,
    },
    EncodingEntry {
        mnemonic: "neg",
        mode: AddrMode::Implied,
        opcode: 0x44,
    },
    EncodingEntry {
        mnemonic: "negx",
        mode: AddrMode::Implied,
        opcode: 0x40,
    },
    EncodingEntry {
        mnemonic: "asl",
        mode: AddrMode::Implied,
        opcode: 0xE1,
    },
    EncodingEntry {
        mnemonic: "asr",
        mode: AddrMode::Implied,
        opcode: 0xE0,
    },
    EncodingEntry {
        mnemonic: "lsl",
        mode: AddrMode::Implied,
        opcode: 0xE3,
    },
    EncodingEntry {
        mnemonic: "lsr",
        mode: AddrMode::Implied,
        opcode: 0xE2,
    },
    EncodingEntry {
        mnemonic: "rol",
        mode: AddrMode::Implied,
        opcode: 0xE7,
    },
    EncodingEntry {
        mnemonic: "ror",
        mode: AddrMode::Implied,
        opcode: 0xE6,
    },
    EncodingEntry {
        mnemonic: "cmp",
        mode: AddrMode::Immediate,
        opcode: 0x0C,
    },
    EncodingEntry {
        mnemonic: "tst",
        mode: AddrMode::Implied,
        opcode: 0x4A,
    },
    EncodingEntry {
        mnemonic: "jmp",
        mode: AddrMode::Absolute,
        opcode: 0x4E,
    },
    EncodingEntry {
        mnemonic: "jsr",
        mode: AddrMode::Absolute,
        opcode: 0x4E,
    },
    EncodingEntry {
        mnemonic: "rts",
        mode: AddrMode::Implied,
        opcode: 0x4E,
    },
    EncodingEntry {
        mnemonic: "rtr",
        mode: AddrMode::Implied,
        opcode: 0x4E,
    },
    EncodingEntry {
        mnemonic: "rte",
        mode: AddrMode::Implied,
        opcode: 0x4E,
    },
    EncodingEntry {
        mnemonic: "bra",
        mode: AddrMode::Relative,
        opcode: 0x60,
    },
    EncodingEntry {
        mnemonic: "bsr",
        mode: AddrMode::Relative,
        opcode: 0x61,
    },
    EncodingEntry {
        mnemonic: "nop",
        mode: AddrMode::Implied,
        opcode: 0x4E,
    },
    EncodingEntry {
        mnemonic: "reset",
        mode: AddrMode::Implied,
        opcode: 0x4E,
    },
    EncodingEntry {
        mnemonic: "stop",
        mode: AddrMode::Immediate,
        opcode: 0x4E,
    },
    EncodingEntry {
        mnemonic: "trap",
        mode: AddrMode::Implied,
        opcode: 0x4E,
    },
    EncodingEntry {
        mnemonic: "swap",
        mode: AddrMode::Implied,
        opcode: 0x48,
    },
    EncodingEntry {
        mnemonic: "ext",
        mode: AddrMode::Implied,
        opcode: 0x48,
    },
    EncodingEntry {
        mnemonic: "and",
        mode: AddrMode::Immediate,
        opcode: 0x02,
    },
    EncodingEntry {
        mnemonic: "or",
        mode: AddrMode::Immediate,
        opcode: 0x00,
    },
    EncodingEntry {
        mnemonic: "eor",
        mode: AddrMode::Immediate,
        opcode: 0x0B,
    },
    EncodingEntry {
        mnemonic: "not",
        mode: AddrMode::Implied,
        opcode: 0x46,
    },
    EncodingEntry {
        mnemonic: "illegal",
        mode: AddrMode::Implied,
        opcode: 0x4A,
    },
];

// --- CPU family selection ---------------------------------------------------

/// Get the encoding table for a given CPU family name.
pub fn get_encoding_table(cpu: &str) -> &'static [EncodingEntry] {
    match cpu {
        "mos6502" => ENCODING_6502,
        "mos65sc02" => ENCODING_65SC02,
        "wdc65c816" => ENCODING_65C816,
        "z80" => ENCODING_Z80,
        "m68000" => ENCODING_68000,
        "rp2A03" => ENCODING_6502,
        "rp2A07" => ENCODING_6502,
        _ => &[],
    }
}

/// Get the full encoding table for a CPU family, including the base 6502
/// table for the 65SC02 and 65C816 (which are supersets of the 6502).
pub fn get_full_encoding_table(cpu: &str) -> Vec<&'static EncodingEntry> {
    let mut table: Vec<&'static EncodingEntry> = Vec::new();
    match cpu {
        "mos6502" => {
            table.extend(ENCODING_6502.iter());
        }
        "mos65sc02" => {
            table.extend(ENCODING_6502.iter());
            table.extend(ENCODING_65SC02.iter());
        }
        "wdc65c816" => {
            table.extend(ENCODING_6502.iter());
            table.extend(ENCODING_65SC02.iter());
            table.extend(ENCODING_65C816.iter());
        }
        "z80" => {
            table.extend(ENCODING_Z80.iter());
        }
        "m68000" => {
            table.extend(ENCODING_68000.iter());
        }
        "rp2A03" => {
            table.extend(ENCODING_6502.iter());
        }
        "rp2A07" => {
            table.extend(ENCODING_6502.iter());
        }
        _ => {}
    }
    table
}
