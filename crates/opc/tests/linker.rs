//! Linker unit tests.
//!
//! These tests call `opc::linker::link_source()` on hand-built
//! `ObjectFile` values and assert the post-link output: patched
//! relocations, merged sections, resolved symbols, vector table entries,
//! section padding, and cleared relocations.

use op_ir::{
    InterruptVector, ObjectFile, RelocKind, Relocation, Section, SectionKind, Symbol, SymbolKind,
};
use opc::linker::link_source;

/// Build an ObjectFile for the NES target with the given sections.
fn obj_with(sections: Vec<Section>) -> ObjectFile {
    ObjectFile {
        version: 1,
        target: "mos6502-nintendo-nes-ntsc".to_string(),
        sections,
        interrupt_vectors: Vec::new(),
        header: None,
        pad_byte: 0x00,
    }
}

/// Build an ObjectFile with sections and a pad_byte.
fn obj_with_pad(sections: Vec<Section>, pad_byte: u8) -> ObjectFile {
    ObjectFile {
        version: 1,
        target: "mos6502-nintendo-nes-ntsc".to_string(),
        sections,
        interrupt_vectors: Vec::new(),
        header: None,
        pad_byte,
    }
}

/// Build an ObjectFile with sections and interrupt vectors.
fn obj_with_vectors(sections: Vec<Section>, vectors: Vec<InterruptVector>) -> ObjectFile {
    ObjectFile {
        version: 1,
        target: "mos6502-nintendo-nes-ntsc".to_string(),
        sections,
        interrupt_vectors: vectors,
        header: None,
        pad_byte: 0x00,
    }
}

/// Build a ROM section with the given name, org, bank, maxsize, data,
/// symbols, and relocations.
fn rom_section(
    name: &str,
    org: u32,
    bank: u32,
    maxsize: u32,
    data: Vec<u8>,
    symbols: Vec<Symbol>,
    relocations: Vec<Relocation>,
) -> Section {
    Section {
        name: name.to_string(),
        kind: SectionKind::Rom,
        org,
        bank,
        maxsize,
        symbols,
        relocations,
        data,
    }
}

/// Build a function symbol.
fn fn_sym(name: &str, offset: u32, size: u32) -> Symbol {
    Symbol {
        name: name.to_string(),
        offset,
        size,
        kind: SymbolKind::Function,
        is_pub: false,
    }
}

/// Build a label symbol.
fn label_sym(name: &str, offset: u32) -> Symbol {
    Symbol {
        name: name.to_string(),
        offset,
        size: 0,
        kind: SymbolKind::Label,
        is_pub: false,
    }
}

/// Build a relocation entry.
fn reloc(offset: u32, kind: RelocKind, symbol: &str) -> Relocation {
    Relocation {
        offset,
        kind,
        symbol: symbol.to_string(),
        addend: 0,
    }
}

/// Build a relocation entry with a non-zero addend.
fn reloc_with_addend(offset: u32, kind: RelocKind, symbol: &str, addend: i64) -> Relocation {
    Relocation {
        offset,
        kind,
        symbol: symbol.to_string(),
        addend,
    }
}

/// Assert no error diagnostics in the linker output. Returns the linked
/// ObjectFile for further assertions.
fn link_clean(obj: &ObjectFile) -> ObjectFile {
    let (linked, diags) = link_source(obj);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "linker errors: {:?}", errors);
    linked
}

// === 1. Relocation resolution =============================================

#[test]
fn link_abs16_relocation() {
    // ROM at 0xC000. fn `helper` is at offset 6 (address 0xC006).
    // A JSR at offset 1 references `helper` with an abs16 reloc at
    // offset 2 (the operand bytes of JSR). After link, the operand
    // bytes must hold 0xC006 in little-endian order.
    let data = vec![0x20, 0x00, 0x00, 0xEA]; // JSR placeholder + NOP
    let symbols = vec![fn_sym("helper", 6, 0), fn_sym("caller", 0, 4)];
    let relocations = vec![reloc(2, RelocKind::Abs16, "helper")];
    let section = rom_section("rom0", 0xC000, 0, 0x4000, data, symbols, relocations);
    let obj = obj_with(vec![section]);

    let linked = link_clean(&obj);
    let s = &linked.sections[0];
    assert!(s.relocations.is_empty(), "relocations should be cleared");
    assert_eq!(s.data[2], 0x06); // low byte of 0xC006
    assert_eq!(s.data[3], 0xC0); // high byte of 0xC006
}

#[test]
fn link_abs16_relocation_with_addend() {
    // ROM at 0xC000. fn `helper` is at offset 6 (address 0xC006).
    // An abs16 reloc with addend 2 patches 0xC008 in little-endian
    // order.
    let data = vec![0x8D, 0x00, 0x00, 0xEA]; // STA placeholder + NOP
    let symbols = vec![fn_sym("helper", 6, 0)];
    let relocations = vec![reloc_with_addend(1, RelocKind::Abs16, "helper", 2)];
    let section = rom_section("rom0", 0xC000, 0, 0x4000, data, symbols, relocations);
    let obj = obj_with(vec![section]);

    let linked = link_clean(&obj);
    let s = &linked.sections[0];
    assert!(s.relocations.is_empty(), "relocations should be cleared");
    assert_eq!(s.data[1], 0x08); // low byte of 0xC008
    assert_eq!(s.data[2], 0xC0); // high byte of 0xC008
}

#[test]
fn link_abs8_relocation() {
    // ROM at 0. Symbol `target` at offset 0x42. An abs8 reloc at
    // offset 0 patches the low byte.
    let data = vec![0x00];
    let symbols = vec![label_sym("target", 0x42)];
    let relocations = vec![reloc(0, RelocKind::Abs8, "target")];
    let section = rom_section("rom0", 0, 0, 0x100, data, symbols, relocations);
    let obj = obj_with(vec![section]);

    let linked = link_clean(&obj);
    assert_eq!(linked.sections[0].data[0], 0x42);
}

#[test]
fn link_branch8_in_range() {
    // ROM at 0. A branch8 reloc at offset 1 references `label` at
    // offset 0x10. The relative offset is 0x10 - (1 + 1) = 0x0E.
    let data = vec![0xF0, 0x00]; // BEQ placeholder
    let symbols = vec![label_sym("label", 0x10)];
    let relocations = vec![reloc(1, RelocKind::Branch8, "label")];
    let section = rom_section("rom0", 0, 0, 0x100, data, symbols, relocations);
    let obj = obj_with(vec![section]);

    let linked = link_clean(&obj);
    assert_eq!(linked.sections[0].data[1], 0x0E);
}

#[test]
fn link_lo8_hi8_relocations() {
    // ROM at 0. Symbol `target` at address 0x1234. lo8 patches the
    // low byte; hi8 patches the high byte.
    let data = vec![0x00, 0x00];
    let symbols = vec![label_sym("target", 0x1234)];
    let relocations = vec![
        reloc(0, RelocKind::Lo8, "target"),
        reloc(1, RelocKind::Hi8, "target"),
    ];
    let section = rom_section("rom0", 0, 0, 0x100, data, symbols, relocations);
    let obj = obj_with(vec![section]);

    let linked = link_clean(&obj);
    assert_eq!(linked.sections[0].data[0], 0x34); // lo8
    assert_eq!(linked.sections[0].data[1], 0x12); // hi8
}

// === 2. Section merging ===================================================

#[test]
fn link_merges_same_name_and_bank() {
    // Two ROM sections with the same name and bank. The linker merges
    // them. The data concatenates. The second section's symbol offset
    // adjusts by the first section's data length.
    let s1 = rom_section(
        "rom0",
        0xC000,
        0,
        0x4000,
        vec![0xA9, 0x00],
        vec![fn_sym("first", 0, 2)],
        vec![],
    );
    let s2 = rom_section(
        "rom0",
        0xC000,
        0,
        0x4000,
        vec![0xEA],
        vec![fn_sym("second", 0, 1)],
        vec![],
    );
    let obj = obj_with(vec![s1, s2]);

    let linked = link_clean(&obj);
    assert_eq!(
        linked.sections.len(),
        1,
        "sections with same name and bank should merge"
    );
    let s = &linked.sections[0];
    // The merged data is the concatenation, then padded to maxsize.
    // Check only the first 3 bytes (the concatenated data).
    assert_eq!(&s.data[..3], &[0xA9, 0x00, 0xEA]);
    assert_eq!(s.data.len(), 0x4000, "merged section should pad to maxsize");
    // `first` stays at offset 0 (address 0xC000).
    // `second` moves to offset 2 (address 0xC002).
    let first = s.symbols.iter().find(|sym| sym.name == "first").unwrap();
    let second = s.symbols.iter().find(|sym| sym.name == "second").unwrap();
    assert_eq!(first.offset, 0);
    assert_eq!(second.offset, 2);
}

#[test]
fn link_does_not_merge_different_banks() {
    let s1 = rom_section("rom0", 0xC000, 0, 0x4000, vec![0xA9], vec![], vec![]);
    let s2 = rom_section("rom0", 0x8000, 1, 0x4000, vec![0xEA], vec![], vec![]);
    let obj = obj_with(vec![s1, s2]);

    let linked = link_clean(&obj);
    assert_eq!(linked.sections.len(), 2, "different banks should not merge");
}

// === 3. Symbol resolution =================================================

#[test]
fn link_symbol_addresses() {
    // ROM at 0xC000. fn `main` at offset 0, fn `helper` at offset 6.
    // After link, the symbol table resolves `helper` to 0xC006.
    let data = vec![0xA9, 0x00, 0x20, 0x00, 0x00, 0xEA, 0xE8];
    let symbols = vec![fn_sym("main", 0, 3), fn_sym("helper", 6, 1)];
    let relocations = vec![reloc(3, RelocKind::Abs16, "helper")];
    let section = rom_section("rom0", 0xC000, 0, 0x4000, data, symbols, relocations);
    let obj = obj_with(vec![section]);

    let linked = link_clean(&obj);
    let s = &linked.sections[0];
    // The abs16 relocation at offset 3 patches with 0xC006.
    assert_eq!(s.data[3], 0x06);
    assert_eq!(s.data[4], 0xC0);
}

#[test]
fn link_resolves_label_symbols() {
    // ROM at 0x8000. Label `start` at offset 0 (address 0x8000). An
    // abs16 relocation patches with 0x8000 in little-endian order.
    let data = vec![0x00, 0x00];
    let symbols = vec![label_sym("start", 0)];
    let relocations = vec![reloc(0, RelocKind::Abs16, "start")];
    let section = rom_section("rom0", 0x8000, 0, 0x100, data, symbols, relocations);
    let obj = obj_with(vec![section]);

    let linked = link_clean(&obj);
    assert_eq!(linked.sections[0].data[0], 0x00);
    assert_eq!(linked.sections[0].data[1], 0x80);
}

// === 4. Branch range check ================================================

#[test]
fn link_branch8_out_of_range() {
    // ROM at 0. branch8 reloc at offset 1 references `far` at offset
    // 0x100. The relative offset is 0x100 - 2 = 0xFE = 254, which is
    // out of the 8-bit signed range (-128..=127).
    let data = vec![0xF0, 0x00];
    let symbols = vec![label_sym("far", 0x100)];
    let relocations = vec![reloc(1, RelocKind::Branch8, "far")];
    let section = rom_section("rom0", 0, 0, 0x200, data, symbols, relocations);
    let obj = obj_with(vec![section]);

    let (_linked, diags) = link_source(&obj);
    let has_error = diags
        .iter()
        .any(|d| d.severity == op_diagnostics::Severity::Error);
    assert!(
        has_error,
        "expected an error diagnostic for out-of-range branch"
    );
}

// === 5. Interrupt vector table ============================================

#[test]
fn link_writes_interrupt_vector_table() {
    // ROM at 0xC000, maxsize 0x4000. fn `main` at offset 0 (address
    // 0xC000). The reset vector at 0xFFFC must hold 0xC000 in
    // little-endian order.
    let data = vec![0xA9, 0x00, 0xEA];
    let symbols = vec![fn_sym("main", 0, 3)];
    let section = rom_section("rom0", 0xC000, 0, 0x4000, data, symbols, vec![]);
    let obj = obj_with_vectors(
        vec![section],
        vec![InterruptVector {
            name: "reset".to_string(),
            address: 0xFFFC,
            target: "main".to_string(),
        }],
    );

    let linked = link_clean(&obj);
    let s = &linked.sections[0];
    let offset = (0xFFFC - 0xC000) as usize;
    assert_eq!(s.data[offset], 0x00); // low byte of 0xC000
    assert_eq!(s.data[offset + 1], 0xC0); // high byte of 0xC000
}

#[test]
fn link_extends_section_for_vector_table() {
    // ROM at 0xC000, maxsize 0x4000, but only 3 bytes of data. The
    // reset vector at 0xFFFC is beyond the current data length. The
    // linker must extend the data to hold the vector entry.
    let data = vec![0xA9, 0x00, 0xEA];
    let symbols = vec![fn_sym("main", 0, 3)];
    let section = rom_section("rom0", 0xC000, 0, 0x4000, data, symbols, vec![]);
    let obj = obj_with_vectors(
        vec![section],
        vec![InterruptVector {
            name: "reset".to_string(),
            address: 0xFFFC,
            target: "main".to_string(),
        }],
    );

    let linked = link_clean(&obj);
    let s = &linked.sections[0];
    let vec_offset = (0xFFFC - 0xC000) as usize;
    assert!(
        s.data.len() >= vec_offset + 2,
        "section data must extend to hold the vector table"
    );
    assert_eq!(s.data[vec_offset], 0x00);
    assert_eq!(s.data[vec_offset + 1], 0xC0);
}

// === 6. Section padding ===================================================

#[test]
fn link_pads_rom_section_to_maxsize() {
    let data = vec![0xA9, 0x00];
    let section = rom_section("rom0", 0xC000, 0, 0x10, data, vec![], vec![]);
    let obj = obj_with(vec![section]);

    let linked = link_clean(&obj);
    assert_eq!(
        linked.sections[0].data.len(),
        0x10,
        "ROM section should pad to maxsize"
    );
    // The padding byte is 0x00 by default.
    assert_eq!(linked.sections[0].data[2], 0x00);
}

#[test]
fn link_pads_with_pad_byte() {
    let data = vec![0xA9, 0x00];
    let section = rom_section("rom0", 0xC000, 0, 0x10, data, vec![], vec![]);
    let obj = obj_with_pad(vec![section], 0xFF);

    let linked = link_clean(&obj);
    assert_eq!(linked.sections[0].data.len(), 0x10);
    assert_eq!(linked.sections[0].data[2], 0xFF);
}

#[test]
fn link_does_not_pad_ram_sections() {
    let ram = Section {
        name: "ram0".to_string(),
        kind: SectionKind::Ram,
        org: 0x0000,
        bank: 0,
        maxsize: 0x100,
        symbols: vec![],
        relocations: vec![],
        data: vec![0x01, 0x02],
    };
    let obj = obj_with(vec![ram]);

    let linked = link_clean(&obj);
    assert_eq!(
        linked.sections[0].data.len(),
        2,
        "RAM sections should not be padded"
    );
}

// === 7. Post-link relocations cleared =====================================

#[test]
fn link_clears_resolved_relocations() {
    let data = vec![0x20, 0x00, 0x00];
    let symbols = vec![fn_sym("target", 0x10, 0)];
    let relocations = vec![reloc(1, RelocKind::Abs16, "target")];
    let section = rom_section("rom0", 0xC000, 0, 0x100, data, symbols, relocations);
    let obj = obj_with(vec![section]);

    let linked = link_clean(&obj);
    assert!(
        linked.sections[0].relocations.is_empty(),
        "resolved relocations should be cleared"
    );
}

// === 8. Full link from codegen ============================================

#[test]
fn link_full_object_from_codegen() {
    // Compile a small source through the codegen, then link it. Assert
    // the linked output has patched data, padded sections, and vector
    // table entries.
    use opc::codegen::compile_source;
    use opc::parser::parse_source;

    let src = r#"
        #[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] {
            #[interrupt(reset)]
            fn main() {
                lda 0
                sta 0x2000
            }
        }
    "#;
    let (ast, parse_diags) = parse_source("test.op", src, "mos6502-nintendo-nes-ntsc", &[]);
    let errors: Vec<_> = parse_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let (obj, codegen_diags) = compile_source(&ast, 1, &[], &[]);
    let errors: Vec<_> = codegen_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "codegen errors: {:?}", errors);

    let linked = link_clean(&obj);

    // The ROM section should be padded to maxsize.
    let rom = linked
        .sections
        .iter()
        .find(|s| s.kind == SectionKind::Rom)
        .expect("expected a ROM section");
    assert_eq!(rom.data.len(), 0x4000, "ROM should pad to maxsize 0x4000");

    // The relocations should be cleared.
    assert!(rom.relocations.is_empty(), "relocations should be cleared");

    // The reset vector at 0xFFFC should hold the address of `main`.
    // `main` is at the start of the ROM (offset 0, address 0xC000).
    let vec_offset = (0xFFFC - 0xC000) as usize;
    assert_eq!(rom.data[vec_offset], 0x00);
    assert_eq!(rom.data[vec_offset + 1], 0xC0);

    // The interrupt_vectors field should pass through.
    assert_eq!(linked.interrupt_vectors.len(), 1);
    assert_eq!(linked.interrupt_vectors[0].name, "reset");
}
