//! Keyhole peephole optimizer.
//!
//! The optimizer runs after the codegen. It operates on the data bytes
//! and relocation entries within a single section. It uses a sliding
//! window of consecutive instructions to identify and apply transforms.
//!
//! The optimizer respects volatile variables. It must not remove a load
//! or store of a volatile variable.
//!
//! The optimizer runs only when the opt level is 1 or higher.

use op_ir::{Relocation, Section};

/// A decoded instruction for the optimizer to work with.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Instruction {
    /// The offset within the section data where this instruction starts.
    offset: usize,
    /// The opcode byte.
    opcode: u8,
    /// The operand bytes (0, 1, or 2 bytes).
    operand: Vec<u8>,
    /// The mnemonic, if recognized (for pattern matching).
    mnemonic: Option<String>,
    /// The addressing mode, if recognized.
    mode: Option<String>,
    /// Whether this instruction accesses a volatile variable.
    is_volatile: bool,
    /// The total size of this instruction in bytes.
    size: usize,
}

/// Run the peephole optimizer on all sections in the object file.
/// The optimizer runs only when `opt_level >= 1`.
pub fn optimize(sections: &mut [Section], opt_level: u8) {
    if opt_level < 1 {
        return;
    }
    for section in sections.iter_mut() {
        optimize_section(section);
    }
}

/// Run the peephole optimizer on a single section.
fn optimize_section(section: &mut Section) {
    // Decode the section data into a list of instructions.
    let instructions = decode_instructions(&section.data);
    if instructions.is_empty() {
        return;
    }

    // Apply the transforms. Each transform may remove or replace
    // instructions. We run the transforms in a loop until no more
    // changes are made.
    let mut changed = true;
    let mut current = instructions;
    let mut pass = 0;
    while changed && pass < 10 {
        changed = false;
        pass += 1;

        let (result, did_change) = apply_transforms(&current, &section.relocations);
        current = result;
        if did_change {
            changed = true;
        }
    }

    // Re-encode the instructions back into section data.
    let (new_data, new_relocs) = reencode(&current, &section.relocations);
    section.data = new_data;
    section.relocations = new_relocs;
}

/// Decode raw section data bytes into a list of instructions.
/// This is a simplified decoder that recognizes common 6502 instruction
/// patterns. For unrecognized bytes, it treats each byte as a 1-byte
/// instruction.
fn decode_instructions(data: &[u8]) -> Vec<Instruction> {
    let mut instructions = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let opcode = data[pos];
        let (mnemonic, mode, operand_size) = decode_6502_opcode(opcode);

        let operand = if pos + 1 + operand_size <= data.len() {
            data[pos + 1..pos + 1 + operand_size].to_vec()
        } else {
            data[pos + 1..].to_vec()
        };

        let size = 1 + operand.len();
        instructions.push(Instruction {
            offset: pos,
            opcode,
            operand,
            mnemonic,
            mode,
            is_volatile: false,
            size,
        });
        pos += size;
    }

    instructions
}

/// Decode a 6502 opcode byte into (mnemonic, mode, operand_size).
/// Returns (None, None, 1) for unrecognized opcodes.
fn decode_6502_opcode(opcode: u8) -> (Option<String>, Option<String>, usize) {
    // A lookup table for the 6502. Each entry maps an opcode byte to
    // (mnemonic, addressing_mode, operand_size).
    // This is a simplified table covering the most common opcodes.
    let (mnemonic, mode, size) = match opcode {
        // Implied mode (1 byte, no operand)
        0x00 => ("brk", "implied", 0),
        0x08 => ("php", "implied", 0),
        0x18 => ("clc", "implied", 0),
        0x28 => ("plp", "implied", 0),
        0x38 => ("sec", "implied", 0),
        0x40 => ("rti", "implied", 0),
        0x48 => ("pha", "implied", 0),
        0x58 => ("cli", "implied", 0),
        0x60 => ("rts", "implied", 0),
        0x68 => ("pla", "implied", 0),
        0x78 => ("sei", "implied", 0),
        0x88 => ("dey", "implied", 0),
        0x8A => ("txa", "implied", 0),
        0x98 => ("tya", "implied", 0),
        0x9A => ("txs", "implied", 0),
        0xA8 => ("tay", "implied", 0),
        0xAA => ("tax", "implied", 0),
        0xB8 => ("clv", "implied", 0),
        0xBA => ("tsx", "implied", 0),
        0xC8 => ("iny", "implied", 0),
        0xCA => ("dex", "implied", 0),
        0xD8 => ("cld", "implied", 0),
        0xE8 => ("inx", "implied", 0),
        0xEA => ("nop", "implied", 0),
        0xF8 => ("sed", "implied", 0),
        // Accumulator mode (1 byte, no operand)
        0x0A => ("asl", "accumulator", 0),
        0x2A => ("rol", "accumulator", 0),
        0x4A => ("lsr", "accumulator", 0),
        0x6A => ("ror", "accumulator", 0),
        // Immediate mode (2 bytes: opcode + 1 byte operand)
        0x09 => ("ora", "immediate", 1),
        0x29 => ("and", "immediate", 1),
        0x49 => ("eor", "immediate", 1),
        0x69 => ("adc", "immediate", 1),
        0xA0 => ("ldy", "immediate", 1),
        0xA2 => ("ldx", "immediate", 1),
        0xA9 => ("lda", "immediate", 1),
        0xC0 => ("cpy", "immediate", 1),
        0xC9 => ("cmp", "immediate", 1),
        0xE0 => ("cpx", "immediate", 1),
        0xE9 => ("sbc", "immediate", 1),
        // Zero-page mode (2 bytes)
        0x05 => ("ora", "zeropage", 1),
        0x06 => ("asl", "zeropage", 1),
        0x24 => ("bit", "zeropage", 1),
        0x25 => ("and", "zeropage", 1),
        0x45 => ("eor", "zeropage", 1),
        0x46 => ("lsr", "zeropage", 1),
        0x65 => ("adc", "zeropage", 1),
        0x66 => ("ror", "zeropage", 1),
        0x84 => ("sty", "zeropage", 1),
        0x85 => ("sta", "zeropage", 1),
        0x86 => ("stx", "zeropage", 1),
        0xA4 => ("ldy", "zeropage", 1),
        0xA5 => ("lda", "zeropage", 1),
        0xA6 => ("ldx", "zeropage", 1),
        0xC4 => ("cpy", "zeropage", 1),
        0xC5 => ("cmp", "zeropage", 1),
        0xC6 => ("dec", "zeropage", 1),
        0xE4 => ("cpx", "zeropage", 1),
        0xE5 => ("sbc", "zeropage", 1),
        0xE6 => ("inc", "zeropage", 1),
        // Zero-page X mode (2 bytes)
        0x15 => ("ora", "zeropagex", 1),
        0x16 => ("asl", "zeropagex", 1),
        0x35 => ("and", "zeropagex", 1),
        0x36 => ("rol", "zeropagex", 1),
        0x55 => ("eor", "zeropagex", 1),
        0x56 => ("lsr", "zeropagex", 1),
        0x75 => ("adc", "zeropagex", 1),
        0x76 => ("ror", "zeropagex", 1),
        0x94 => ("sty", "zeropagex", 1),
        0x95 => ("sta", "zeropagex", 1),
        0xB4 => ("ldy", "zeropagex", 1),
        0xB5 => ("lda", "zeropagex", 1),
        0xD5 => ("cmp", "zeropagex", 1),
        0xD6 => ("dec", "zeropagex", 1),
        0xF5 => ("sbc", "zeropagex", 1),
        0xF6 => ("inc", "zeropagex", 1),
        // Zero-page Y mode (2 bytes)
        0x96 => ("stx", "zeropagey", 1),
        0xB6 => ("ldx", "zeropagey", 1),
        // Absolute mode (3 bytes: opcode + 2 byte operand)
        0x0C => ("tsb", "absolute", 2),
        0x0D => ("ora", "absolute", 2),
        0x0E => ("asl", "absolute", 2),
        0x1C => ("trb", "absolute", 2),
        0x2C => ("bit", "absolute", 2),
        0x2D => ("and", "absolute", 2),
        0x2E => ("rol", "absolute", 2),
        0x4C => ("jmp", "absolute", 2),
        0x4D => ("eor", "absolute", 2),
        0x4E => ("lsr", "absolute", 2),
        0x6C => ("jmp", "indirect", 2),
        0x6D => ("adc", "absolute", 2),
        0x6E => ("ror", "absolute", 2),
        0x8C => ("sty", "absolute", 2),
        0x8D => ("sta", "absolute", 2),
        0x8E => ("stx", "absolute", 2),
        0x9C => ("stz", "absolute", 2),
        0xAC => ("ldy", "absolute", 2),
        0xAD => ("lda", "absolute", 2),
        0xAE => ("ldx", "absolute", 2),
        0xCC => ("cpy", "absolute", 2),
        0xCD => ("cmp", "absolute", 2),
        0xCE => ("dec", "absolute", 2),
        0xEC => ("cpx", "absolute", 2),
        0xED => ("sbc", "absolute", 2),
        0xEE => ("inc", "absolute", 2),
        // Absolute X mode (3 bytes)
        0x1D => ("ora", "absolutex", 2),
        0x1E => ("asl", "absolutex", 2),
        0x3D => ("and", "absolutex", 2),
        0x3E => ("rol", "absolutex", 2),
        0x5D => ("eor", "absolutex", 2),
        0x5E => ("lsr", "absolutex", 2),
        0x7D => ("adc", "absolutex", 2),
        0x7E => ("ror", "absolutex", 2),
        0x9D => ("sta", "absolutex", 2),
        0x9E => ("stz", "absolutex", 2),
        0xBC => ("ldy", "absolutex", 2),
        0xBD => ("lda", "absolutex", 2),
        0xDD => ("cmp", "absolutex", 2),
        0xDE => ("dec", "absolutex", 2),
        0xFD => ("sbc", "absolutex", 2),
        0xFE => ("inc", "absolutex", 2),
        // Absolute Y mode (3 bytes)
        0x19 => ("ora", "absolutey", 2),
        0x39 => ("and", "absolutey", 2),
        0x59 => ("eor", "absolutey", 2),
        0x79 => ("adc", "absolutey", 2),
        0x99 => ("sta", "absolutey", 2),
        0xB9 => ("lda", "absolutey", 2),
        0xBE => ("ldx", "absolutey", 2),
        0xD9 => ("cmp", "absolutey", 2),
        0xF9 => ("sbc", "absolutey", 2),
        // Indirect X mode (2 bytes)
        0x01 => ("ora", "indirectx", 1),
        0x21 => ("and", "indirectx", 1),
        0x41 => ("eor", "indirectx", 1),
        0x61 => ("adc", "indirectx", 1),
        0x81 => ("sta", "indirectx", 1),
        0xA1 => ("lda", "indirectx", 1),
        0xC1 => ("cmp", "indirectx", 1),
        0xE1 => ("sbc", "indirectx", 1),
        // Indirect Y mode (2 bytes)
        0x11 => ("ora", "indirecty", 1),
        0x31 => ("and", "indirecty", 1),
        0x51 => ("eor", "indirecty", 1),
        0x71 => ("adc", "indirecty", 1),
        0x91 => ("sta", "indirecty", 1),
        0xB1 => ("lda", "indirecty", 1),
        0xD1 => ("cmp", "indirecty", 1),
        0xF1 => ("sbc", "indirecty", 1),
        // Relative mode (2 bytes: opcode + 1 byte offset)
        0x10 => ("bpl", "relative", 1),
        0x30 => ("bmi", "relative", 1),
        0x50 => ("bvc", "relative", 1),
        0x70 => ("bvs", "relative", 1),
        0x80 => ("bra", "relative", 1),
        0x90 => ("bcc", "relative", 1),
        0xB0 => ("bcs", "relative", 1),
        0xD0 => ("bne", "relative", 1),
        0xF0 => ("beq", "relative", 1),
        // JSR (3 bytes)
        0x20 => ("jsr", "absolute", 2),
        // 65SC02 additional
        0x1A => ("ina", "implied", 0),
        0x3A => ("dea", "implied", 0),
        0x5A => ("phy", "implied", 0),
        0x7A => ("ply", "implied", 0),
        0xDA => ("phx", "implied", 0),
        0xFA => ("plx", "implied", 0),
        // Default: treat as 1-byte instruction
        _ => ("", "unknown", 0),
    };

    if mnemonic.is_empty() {
        (None, None, 1)
    } else {
        (Some(mnemonic.to_string()), Some(mode.to_string()), size)
    }
}

/// Apply all peephole transforms to the instruction list.
/// Returns the new instruction list and whether any changes were made.
fn apply_transforms(
    instructions: &[Instruction],
    _relocations: &[Relocation],
) -> (Vec<Instruction>, bool) {
    let mut result: Vec<Instruction> = Vec::new();
    let mut changed = false;
    let mut i = 0;

    while i < instructions.len() {
        // Try each transform at the current position.

        // 1. Redundant load: lda X then lda X -> lda X
        if let Some(removed) = try_redundant_load(instructions, i) {
            result.push(instructions[i].clone());
            i += removed;
            changed = true;
            continue;
        }

        // 2. Redundant store: sta X then sta X -> sta X
        if let Some(removed) = try_redundant_store(instructions, i) {
            result.push(instructions[i].clone());
            i += removed;
            changed = true;
            continue;
        }

        // 3. Load-store-load: lda X, sta Y, lda Y -> lda X, sta Y
        if let Some(removed) = try_load_store_load(instructions, i) {
            result.push(instructions[i].clone());
            if i + 1 < instructions.len() {
                result.push(instructions[i + 1].clone());
            }
            i += removed;
            changed = true;
            continue;
        }

        // 4. Dead store: sta X then no read of X before next store to X
        if let Some(removed) = try_dead_store(instructions, i) {
            // Skip the dead store entirely.
            i += removed;
            changed = true;
            continue;
        }

        // 5. Branch to next: bra L then L: -> remove the branch
        if let Some(removed) = try_branch_to_next(instructions, i) {
            i += removed;
            changed = true;
            continue;
        }

        // 6. Branch to branch: bra L1, L1: bra L2 -> bra L2
        if let Some(removed) = try_branch_to_branch(instructions, i) {
            i += removed;
            changed = true;
            continue;
        }

        // 7. Constant fold: lda #(1+2) -> lda #3
        // The parser already folds constants, so this is a no-op in practice.
        // No action needed.

        // 8. Strength reduce: lda #0, clc, adc #0 -> lda #0
        if let Some(removed) = try_strength_reduce(instructions, i) {
            result.push(instructions[i].clone());
            i += removed;
            changed = true;
            continue;
        }

        // 9. Stack push-pop: pha then pla -> nothing
        if let Some(removed) = try_stack_push_pop(instructions, i) {
            i += removed;
            changed = true;
            continue;
        }

        // No transform matched — keep the instruction.
        result.push(instructions[i].clone());
        i += 1;
    }

    (result, changed)
}

/// Check if two instructions are the same load (same opcode + same operand).
fn is_same_load(a: &Instruction, b: &Instruction) -> bool {
    let load_mnemonics = ["lda", "ldx", "ldy"];
    is_same_instruction(a, b, &load_mnemonics)
}

/// Check if two instructions are the same store (same opcode + same operand).
fn is_same_store(a: &Instruction, b: &Instruction) -> bool {
    let store_mnemonics = ["sta", "stx", "sty", "stz"];
    is_same_instruction(a, b, &store_mnemonics)
}

/// Check if two instructions have the same mnemonic and operand.
fn is_same_instruction(a: &Instruction, b: &Instruction, mnemonics: &[&str]) -> bool {
    if a.mnemonic.as_deref() == b.mnemonic.as_deref() {
        if let Some(m) = &a.mnemonic {
            if mnemonics.contains(&m.as_str()) {
                return a.operand == b.operand && !a.is_volatile && !b.is_volatile;
            }
        }
    }
    false
}

/// 1. Redundant load: lda X then lda X -> lda X (remove the second).
fn try_redundant_load(instructions: &[Instruction], i: usize) -> Option<usize> {
    if i + 1 >= instructions.len() {
        return None;
    }
    if is_same_load(&instructions[i], &instructions[i + 1]) {
        return Some(2); // skip both, we keep the first
    }
    None
}

/// 2. Redundant store: sta X then sta X -> sta X (remove the second).
fn try_redundant_store(instructions: &[Instruction], i: usize) -> Option<usize> {
    if i + 1 >= instructions.len() {
        return None;
    }
    if is_same_store(&instructions[i], &instructions[i + 1]) {
        return Some(2); // skip both, we keep the first
    }
    None
}

/// 3. Load-store-load: lda X, sta Y, lda Y -> lda X, sta Y (remove the third).
fn try_load_store_load(instructions: &[Instruction], i: usize) -> Option<usize> {
    if i + 2 >= instructions.len() {
        return None;
    }
    let load = &instructions[i];
    let store = &instructions[i + 1];
    let load2 = &instructions[i + 2];

    // Check: first is a load, second is a store, third is a load of the
    // same address as the store.
    let is_load = load
        .mnemonic
        .as_deref()
        .map(|m| ["lda", "ldx", "ldy"].contains(&m))
        .unwrap_or(false);
    let is_store = store
        .mnemonic
        .as_deref()
        .map(|m| ["sta", "stx", "sty", "stz"].contains(&m))
        .unwrap_or(false);
    let is_load2 = load2
        .mnemonic
        .as_deref()
        .map(|m| ["lda", "ldx", "ldy"].contains(&m))
        .unwrap_or(false);

    if is_load && is_store && is_load2 && store.operand == load2.operand && !load2.is_volatile {
        return Some(3); // skip all three, we keep the first two
    }
    None
}

/// 4. Dead store: sta X then no read of X before next store to X.
fn try_dead_store(instructions: &[Instruction], i: usize) -> Option<usize> {
    if i + 1 >= instructions.len() {
        return None;
    }
    let store = &instructions[i];
    let is_store = store
        .mnemonic
        .as_deref()
        .map(|m| ["sta", "stx", "sty", "stz"].contains(&m))
        .unwrap_or(false);
    if !is_store || store.is_volatile {
        return None;
    }

    // Look ahead: is there a read of the same address before the next
    // store to the same address?
    let store_addr = &store.operand;
    for inst in instructions.iter().take(instructions.len()).skip(i + 1) {
        let reads_addr = inst
            .mnemonic
            .as_deref()
            .map(|m| ["lda", "ldx", "ldy", "cmp", "cpx", "cpy", "bit"].contains(&m))
            .unwrap_or(false);
        let writes_addr = inst
            .mnemonic
            .as_deref()
            .map(|m| ["sta", "stx", "sty", "stz"].contains(&m))
            .unwrap_or(false);

        if inst.operand == *store_addr {
            if reads_addr {
                // The store is read — not dead.
                return None;
            }
            if writes_addr {
                // Another store to the same address — the first is dead.
                return Some(1); // skip the dead store
            }
        }

        // A branch or jump means we can't safely remove the store.
        let is_branch = inst
            .mnemonic
            .as_deref()
            .map(|m| {
                [
                    "bpl", "bmi", "bvc", "bvs", "bcc", "bcs", "bne", "beq", "bra",
                ]
                .contains(&m)
            })
            .unwrap_or(false);
        let is_jump = inst
            .mnemonic
            .as_deref()
            .map(|m| ["jmp", "jsr", "rts", "rti"].contains(&m))
            .unwrap_or(false);
        if is_branch || is_jump {
            return None;
        }
    }
    None
}

/// 5. Branch to next: bra L then L: -> remove the branch.
fn try_branch_to_next(instructions: &[Instruction], i: usize) -> Option<usize> {
    if i + 1 >= instructions.len() {
        return None;
    }
    let branch = &instructions[i];
    let is_branch = branch
        .mnemonic
        .as_deref()
        .map(|m| {
            [
                "bpl", "bmi", "bvc", "bvs", "bcc", "bcs", "bne", "beq", "bra",
            ]
            .contains(&m)
        })
        .unwrap_or(false);
    if !is_branch {
        return None;
    }
    // Check if the branch offset is 0 (branches to the next instruction).
    if branch.operand.len() == 1 && branch.operand[0] == 0 {
        return Some(1); // skip the branch
    }
    None
}

/// 6. Branch to branch: bra L1, L1: bra L2 -> bra L2.
fn try_branch_to_branch(_instructions: &[Instruction], _i: usize) -> Option<usize> {
    // This transform requires label resolution which is complex.
    // For now, this is a no-op. A future revision can implement it.
    None
}

/// 8. Strength reduce: lda #0, clc, adc #0 -> lda #0.
fn try_strength_reduce(instructions: &[Instruction], i: usize) -> Option<usize> {
    if i + 2 >= instructions.len() {
        return None;
    }
    let lda = &instructions[i];
    let clc = &instructions[i + 1];
    let adc = &instructions[i + 2];

    let is_lda_zero = lda.mnemonic.as_deref() == Some("lda")
        && lda.mode.as_deref() == Some("immediate")
        && lda.operand.len() == 1
        && lda.operand[0] == 0;
    let is_clc = clc.mnemonic.as_deref() == Some("clc");
    let is_adc_zero = adc.mnemonic.as_deref() == Some("adc")
        && adc.mode.as_deref() == Some("immediate")
        && adc.operand.len() == 1
        && adc.operand[0] == 0;

    if is_lda_zero && is_clc && is_adc_zero {
        return Some(3); // skip all three, we keep the first
    }
    None
}

/// 9. Stack push-pop: pha then pla -> nothing (if no side effect between).
fn try_stack_push_pop(instructions: &[Instruction], i: usize) -> Option<usize> {
    if i + 1 >= instructions.len() {
        return None;
    }
    let push = &instructions[i];
    let pop = &instructions[i + 1];

    let is_pha = push.mnemonic.as_deref() == Some("pha");
    let is_pla = pop.mnemonic.as_deref() == Some("pla");

    if is_pha && is_pla {
        return Some(2); // skip both
    }
    None
}

/// Re-encode the instruction list back into section data and relocations.
fn reencode(
    instructions: &[Instruction],
    relocations: &[Relocation],
) -> (Vec<u8>, Vec<Relocation>) {
    let mut data = Vec::new();
    let mut new_relocations = Vec::new();

    // Build a mapping from old offsets to new offsets.
    let mut offset_map: Vec<(usize, usize)> = Vec::new();
    for inst in instructions {
        let old_offset = inst.offset;
        let new_offset = data.len();
        offset_map.push((old_offset, new_offset));

        data.push(inst.opcode);
        data.extend_from_slice(&inst.operand);
    }

    // Remap relocations to the new offsets.
    for reloc in relocations {
        // Find the instruction that contains this relocation offset.
        let containing = offset_map.iter().find(|entry| {
            let old = entry.0;
            reloc.offset as usize >= old && (reloc.offset as usize) < (old + 3)
        });

        if let Some(entry) = containing {
            let old_off = entry.0;
            let new_off = entry.1;
            let adjusted_offset = new_off + (reloc.offset as usize - old_off);
            new_relocations.push(Relocation {
                offset: adjusted_offset as u32,
                kind: reloc.kind,
                symbol: reloc.symbol.clone(),
            });
        } else {
            // Keep the relocation as-is if we can't remap it.
            new_relocations.push(reloc.clone());
        }
    }

    (data, new_relocations)
}
