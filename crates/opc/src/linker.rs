//! Stage 4: linker.
//!
//! The linker reads the post-compile object data, resolves all
//! relocations, merges sections, writes interrupt vector tables, pads
//! sections to their maxsize, and produces the post-link object data.

use anyhow::Result;
use op_diagnostics::{Diagnostic, Severity};
use op_ir::{InterruptVector, ObjectFile, RelocKind, Section, SectionKind};
use std::collections::HashMap;

use crate::cli::OpcArgs;

/// Run the linker stage when the `--link` flag is set.
pub fn run(args: &OpcArgs) -> Result<()> {
    if !args.link {
        return Ok(());
    }
    let target = args.target.as_deref().unwrap_or("");
    let linked = link_file(&args.input.input, target)?;
    let json = op_common::to_json(&linked)?;
    match &args.output {
        Some(path) => std::fs::write(path, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

/// Read a `.opl` JSON file, link it, and return the post-link
/// [`ObjectFile`].
pub fn link_file(path: &str, target: &str) -> Result<ObjectFile> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path))?;
    let obj: ObjectFile = op_common::from_json(&source)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path))?;
    let (linked, diags) = link_source(&obj);
    let has_errors = diags.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        for d in &diags {
            d.print(None);
        }
        anyhow::bail!("linker errors in {}", path);
    }
    let _ = target; // target is already in the ObjectFile
    Ok(linked)
}

/// Link a post-compile [`ObjectFile`] into a post-link [`ObjectFile`].
///
/// This is the main entry point for the linker. It resolves all
/// relocations, merges sections, writes interrupt vector tables, and
/// pads sections.
pub fn link_source(obj: &ObjectFile) -> (ObjectFile, Vec<Diagnostic>) {
    let mut linker = Linker { diags: Vec::new() };

    // Step 1: Collect sections (clone from input).
    let mut sections: Vec<Section> = obj.sections.clone();

    // Step 2: Merge sections with the same name and bank.
    sections = linker.merge_sections(sections);

    // Step 3: Resolve — build the symbol table.
    let symbol_table = linker.build_symbol_table(&sections);

    // Step 4: Patch relocations.
    linker.patch_relocations(&mut sections, &symbol_table);

    // Step 6: Write interrupt vector table.
    linker.write_vector_tables(&mut sections, &obj.interrupt_vectors, &symbol_table);

    // Step 8: Pad sections.
    linker.pad_sections(&mut sections, obj.pad_byte);

    // Build the post-link ObjectFile.
    let linked = ObjectFile {
        version: obj.version,
        target: obj.target.clone(),
        sections,
        interrupt_vectors: obj.interrupt_vectors.clone(),
        header: obj.header.clone(),
        pad_byte: obj.pad_byte,
    };

    (linked, linker.diags)
}

// --- Linker struct ----------------------------------------------------------

struct Linker {
    diags: Vec<Diagnostic>,
}

impl Linker {
    // Step 2: Merge sections with the same name and bank.
    fn merge_sections(&mut self, sections: Vec<Section>) -> Vec<Section> {
        let mut merged: Vec<Section> = Vec::new();
        for section in sections {
            // Find an existing section with the same name and bank.
            if let Some(idx) = merged
                .iter()
                .position(|s| s.name == section.name && s.bank == section.bank)
            {
                let existing = &mut merged[idx];
                let old_data_len = existing.data.len() as u32;

                // Adjust symbol offsets for the merged section.
                for sym in &mut existing.symbols {
                    // Symbols already have their offsets relative to the
                    // section origin. When we concatenate data, the new
                    // symbols need their offsets adjusted by the old data
                    // length.
                    let _ = sym; // offsets are already correct for the
                                 // existing section; new symbols below.
                }

                // Adjust the new section's symbols.
                let mut new_section = section;
                for sym in &mut new_section.symbols {
                    sym.offset += old_data_len;
                }

                // Adjust relocations in the new section.
                for reloc in &mut new_section.relocations {
                    reloc.offset += old_data_len;
                }

                // Concatenate data.
                existing.data.extend_from_slice(&new_section.data);
                existing.symbols.extend(new_section.symbols);
                existing.relocations.extend(new_section.relocations);
            } else {
                merged.push(section);
            }
        }
        merged
    }

    // Step 3: Build the symbol table.
    fn build_symbol_table(&self, sections: &[Section]) -> HashMap<String, u32> {
        let mut table = HashMap::new();
        for section in sections {
            for sym in &section.symbols {
                let addr = section.org + sym.offset;
                table.insert(sym.name.clone(), addr);
            }
        }
        table
    }

    // Step 4: Patch relocations.
    fn patch_relocations(&mut self, sections: &mut [Section], symbol_table: &HashMap<String, u32>) {
        for section in sections.iter_mut() {
            let section_org = section.org;
            let mut remaining_relocs = Vec::new();

            for reloc in &section.relocations {
                let target_addr = match symbol_table.get(&reloc.symbol) {
                    Some(addr) => *addr,
                    None => {
                        // Try parsing the symbol as a numeric literal.
                        if let Some(addr) = parse_numeric(&reloc.symbol) {
                            addr
                        } else {
                            self.diags.push(Diagnostic::error(
                                400,
                                "",
                                0,
                                0,
                                format!("unresolved symbol: '{}'", reloc.symbol),
                            ));
                            remaining_relocs.push(reloc.clone());
                            continue;
                        }
                    }
                };

                let offset = reloc.offset as usize;
                let data = &mut section.data;

                match reloc.kind {
                    RelocKind::Abs8 => {
                        if offset < data.len() {
                            data[offset] = (target_addr & 0xFF) as u8;
                        }
                    }
                    RelocKind::Abs16 => {
                        if offset + 1 < data.len() {
                            data[offset] = (target_addr & 0xFF) as u8;
                            data[offset + 1] = ((target_addr >> 8) & 0xFF) as u8;
                        }
                    }
                    RelocKind::Abs24 => {
                        if offset + 2 < data.len() {
                            data[offset] = (target_addr & 0xFF) as u8;
                            data[offset + 1] = ((target_addr >> 8) & 0xFF) as u8;
                            data[offset + 2] = ((target_addr >> 16) & 0xFF) as u8;
                        }
                    }
                    RelocKind::Abs32 => {
                        if offset + 3 < data.len() {
                            data[offset] = (target_addr & 0xFF) as u8;
                            data[offset + 1] = ((target_addr >> 8) & 0xFF) as u8;
                            data[offset + 2] = ((target_addr >> 16) & 0xFF) as u8;
                            data[offset + 3] = ((target_addr >> 24) & 0xFF) as u8;
                        }
                    }
                    RelocKind::Branch8 => {
                        // Relative offset: target - (reloc_site + 1)
                        let reloc_site_addr = section_org + reloc.offset;
                        let relative = target_addr as i64 - (reloc_site_addr as i64 + 1);
                        if !(-128..=127).contains(&relative) {
                            self.diags.push(Diagnostic::error(
                                401,
                                "",
                                0,
                                0,
                                format!(
                                    "branch out of range: {} (target={}, site={})",
                                    relative, target_addr, reloc_site_addr
                                ),
                            ));
                            remaining_relocs.push(reloc.clone());
                        } else if offset < data.len() {
                            data[offset] = (relative & 0xFF) as u8;
                        }
                    }
                    RelocKind::Branch16 => {
                        let reloc_site_addr = section_org + reloc.offset;
                        let relative = target_addr as i64 - (reloc_site_addr as i64 + 2);
                        if !(-32768..=32767).contains(&relative) {
                            self.diags.push(Diagnostic::error(
                                401,
                                "",
                                0,
                                0,
                                format!(
                                    "branch16 out of range: {} (target={}, site={})",
                                    relative, target_addr, reloc_site_addr
                                ),
                            ));
                            remaining_relocs.push(reloc.clone());
                        } else if offset + 1 < data.len() {
                            data[offset] = (relative & 0xFF) as u8;
                            data[offset + 1] = ((relative >> 8) & 0xFF) as u8;
                        }
                    }
                    RelocKind::Lo8 => {
                        if offset < data.len() {
                            data[offset] = (target_addr & 0xFF) as u8;
                        }
                    }
                    RelocKind::Hi8 => {
                        if offset < data.len() {
                            data[offset] = ((target_addr >> 8) & 0xFF) as u8;
                        }
                    }
                    RelocKind::Bank => {
                        if offset < data.len() {
                            // The bank number is derived from the section
                            // the symbol is in. For now, use 0.
                            data[offset] = 0;
                        }
                    }
                }
            }

            // Replace relocations with only the unresolved ones.
            section.relocations = remaining_relocs;
        }
    }

    // Step 6: Write interrupt vector table entries.
    fn write_vector_tables(
        &mut self,
        sections: &mut [Section],
        vectors: &[InterruptVector],
        symbol_table: &HashMap<String, u32>,
    ) {
        for vector in vectors {
            // Look up the target function address.
            let target_addr = match symbol_table.get(&vector.target) {
                Some(addr) => *addr,
                None => {
                    self.diags.push(Diagnostic::error(
                        402,
                        "",
                        0,
                        0,
                        format!("interrupt vector target not found: '{}'", vector.target),
                    ));
                    continue;
                }
            };

            // Find the ROM section that contains the vector address.
            let vec_addr = vector.address;
            let rom_section = sections.iter_mut().find(|s| {
                s.kind == SectionKind::Rom && vec_addr >= s.org && vec_addr < s.org + s.maxsize
            });

            if let Some(section) = rom_section {
                let offset = (vec_addr - section.org) as usize;

                // Ensure the section data is large enough.
                let needed = offset + 2;
                if section.data.len() < needed {
                    section.data.resize(needed, 0);
                }

                // Write the 2-byte address in little-endian order.
                section.data[offset] = (target_addr & 0xFF) as u8;
                section.data[offset + 1] = ((target_addr >> 8) & 0xFF) as u8;
            } else {
                self.diags.push(Diagnostic::error(
                    403,
                    "",
                    0,
                    0,
                    format!(
                        "no ROM section contains vector address 0x{:04X} for interrupt '{}'",
                        vec_addr, vector.name
                    ),
                ));
            }
        }
    }

    // Step 8: Pad sections to their maxsize.
    fn pad_sections(&mut self, sections: &mut [Section], pad_byte: u8) {
        for section in sections.iter_mut() {
            // Only pad ROM and CHR sections.
            if section.kind == SectionKind::Ram {
                continue;
            }
            if section.maxsize > 0 && (section.data.len() as u32) < section.maxsize {
                let needed = section.maxsize as usize;
                section.data.resize(needed, pad_byte);
            }
        }
    }
}

// --- Helper functions -------------------------------------------------------

/// Try to parse a string as a numeric literal (decimal or hex).
fn parse_numeric(s: &str) -> Option<u32> {
    let s = s.trim_matches('"');
    if let Some(hex) = s.strip_prefix("0x") {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u32>().ok()
    }
}
