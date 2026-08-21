//! Stage 5: file output.
//!
//! When `opc` runs without a stage flag, the file output stage reads the
//! linked object data and writes the final ROM or binary image. The output
//! format depends on the target or the `--format` flag: `ines`, `lnx`,
//! `raw`, `hex`, `sega`, `snes`, `gb`, `sms`, or `a78`.

use anyhow::Result;
use op_ir::{ObjectFile, Section, SectionKind};

use crate::cli::OpcArgs;

/// Run the file output stage when no stage flag is set.
///
/// When the in-memory pipeline is active (Phase 6), this entry point is
/// replaced by a direct call to [`emit_linked`]. For standalone use, this
/// function reads a linked `.opl` JSON file from the input path, emits the
/// final binary, and writes it to the output file or stdout.
pub fn run(args: &OpcArgs) -> Result<()> {
    if args.lex || args.parse || args.compile || args.link {
        return Ok(());
    }

    // Read the linked .opl file.
    let source = std::fs::read_to_string(&args.input.input)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", args.input.input))?;
    let obj: ObjectFile = op_common::from_json(&source)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", args.input.input))?;

    let format = resolve_format(args, &obj.target);
    let bytes = emit_linked(&obj, &format)?;

    match &args.output {
        Some(path) => std::fs::write(path, bytes)?,
        None => {
            use std::io::Write;
            std::io::stdout().write_all(&bytes)?;
        }
    }
    Ok(())
}

/// Resolve the output format from the `--format` flag or the target triplet.
pub fn resolve_format(args: &OpcArgs, target: &str) -> String {
    if let Some(fmt) = &args.format {
        return fmt.clone();
    }
    default_format_for_target(target).to_string()
}

/// Map a target triplet to its default output format.
pub fn default_format_for_target(target: &str) -> &'static str {
    // Parse the triplet: cpu-manufacturer-machine-variant.
    let parts: Vec<&str> = target.split('-').collect();
    let machine = parts.get(2).copied().unwrap_or("");
    let cpu = parts.first().copied().unwrap_or("");

    match machine {
        "nes" => "ines",
        "lynx" => "lnx",
        "genesis" | "megadrive" => "sega",
        "snes" | "sfc" => "snes",
        "gameboy" | "gb" => "gb",
        "gbc" => "gb",
        "sms" | "mastersystem" => "sms",
        "gamegear" | "gg" => "sms",
        "sg1000" => "sms",
        "a7800" | "atari7800" => "a78",
        // Apple II, Atari 800, C64, PCE, Neo Geo, ZX, TI-85: raw binary.
        _ => {
            // Lynx can also be detected by cpu when machine is missing.
            if cpu == "65c02" && machine.is_empty() {
                "lnx"
            } else {
                "raw"
            }
        }
    }
}

/// Emit the final binary image bytes for the given format.
///
/// This is the main entry point for the file output stage. It takes a
/// post-link [`ObjectFile`] and a format string, and returns the final
/// binary image bytes.
pub fn emit_linked(obj: &ObjectFile, format: &str) -> Result<Vec<u8>> {
    match format {
        "ines" => emit_ines(obj),
        "lnx" => emit_lnx(obj),
        "raw" => Ok(emit_raw(obj)),
        "hex" | "ihex" | "intelhex" => emit_intel_hex(obj),
        "sega" => emit_sega(obj),
        "snes" => Ok(emit_snes(obj)),
        "gb" => Ok(emit_gb(obj)),
        "sms" => Ok(emit_sms(obj)),
        "a78" => emit_a78(obj),
        other => Err(anyhow::anyhow!("unknown output format: '{}'", other)),
    }
}

// --- Section collection helpers --------------------------------------------

/// Collect all ROM sections sorted by bank number.
fn rom_sections(obj: &ObjectFile) -> Vec<&Section> {
    let mut roms: Vec<&Section> = obj
        .sections
        .iter()
        .filter(|s| s.kind == SectionKind::Rom)
        .collect();
    roms.sort_by_key(|s| s.bank);
    roms
}

/// Collect all CHR sections sorted by bank number.
fn chr_sections(obj: &ObjectFile) -> Vec<&Section> {
    let mut chrs: Vec<&Section> = obj
        .sections
        .iter()
        .filter(|s| s.kind == SectionKind::Chr)
        .collect();
    chrs.sort_by_key(|s| s.bank);
    chrs
}

/// Look up a header field value by key.
fn header_field<'a>(obj: &'a ObjectFile, key: &str) -> Option<&'a str> {
    obj.header.as_ref().and_then(|h| {
        h.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    })
}

/// Parse a header field as u32 (decimal or hex).
fn header_field_u32(obj: &ObjectFile, key: &str) -> Option<u32> {
    header_field(obj, key).and_then(|v| {
        if let Some(hex) = v.strip_prefix("0x") {
            u32::from_str_radix(hex, 16).ok()
        } else {
            v.parse::<u32>().ok()
        }
    })
}

/// Parse a header field as a boolean.
fn header_field_bool(obj: &ObjectFile, key: &str) -> bool {
    matches!(header_field(obj, key), Some("true") | Some("1"))
}

// --- iNES format ----------------------------------------------------------

/// Emit an iNES (.nes) file.
///
/// The 16-byte header is followed by the PRG ROM data, then the CHR ROM
/// data.
fn emit_ines(obj: &ObjectFile) -> Result<Vec<u8>> {
    let roms = rom_sections(obj);
    let chrs = chr_sections(obj);

    let prg_bytes: Vec<u8> = roms.iter().flat_map(|s| s.data.iter().copied()).collect();
    let chr_bytes: Vec<u8> = chrs.iter().flat_map(|s| s.data.iter().copied()).collect();

    // PRG size in 16KB units. Round up.
    let prg_units = bytes_to_units(prg_bytes.len(), 16 * 1024);
    // CHR size in 8KB units. Round up.
    let chr_units = bytes_to_units(chr_bytes.len(), 8 * 1024);

    // Mapper number from header fields or 0.
    let mapper = header_field_u32(obj, "mapper").unwrap_or(0);
    let mapper_lo = (mapper & 0x0F) as u8;
    let mapper_hi = ((mapper >> 4) & 0x0F) as u8;

    // Flags 6: lower mapper nibble | mirroring | battery | trainer | fourscreen.
    let mut flags6 = mapper_lo << 4;
    if header_field_bool(obj, "battery") {
        flags6 |= 0x02;
    }
    if header_field_bool(obj, "trainer") {
        flags6 |= 0x04;
    }
    if header_field_bool(obj, "fourscreen") {
        flags6 |= 0x08;
    }
    // Mirroring: "horizontal" sets bit 0; "vertical" clears it.
    match header_field(obj, "mirroring") {
        Some("horizontal") => flags6 |= 0x01,
        Some("vertical") => {}
        _ => {}
    }

    // Flags 7: upper mapper nibble. NES 1.0 leaves the rest as 0.
    let flags7 = mapper_hi << 4;

    let mut out = Vec::with_capacity(16 + prg_bytes.len() + chr_bytes.len());
    // Magic: "NES" + 0x1A.
    out.extend_from_slice(&[b'N', b'E', b'S', 0x1A]);
    out.push(prg_units);
    out.push(chr_units);
    out.push(flags6);
    out.push(flags7);
    // Flags 8-15: zero (NES 1.0).
    out.extend_from_slice(&[0; 8]);

    // PRG ROM data, padded to a whole 16KB unit.
    let prg_padded_len = prg_units as usize * 16 * 1024;
    out.extend_from_slice(&prg_bytes);
    if out.len() - 16 < prg_padded_len {
        let pad = prg_padded_len - (out.len() - 16);
        out.extend(std::iter::repeat_n(obj.pad_byte, pad));
    }

    // CHR ROM data, padded to a whole 8KB unit.
    let chr_padded_len = chr_units as usize * 8 * 1024;
    let chr_start = out.len();
    out.extend_from_slice(&chr_bytes);
    if out.len() - chr_start < chr_padded_len {
        let pad = chr_padded_len - (out.len() - chr_start);
        out.extend(std::iter::repeat_n(obj.pad_byte, pad));
    }

    Ok(out)
}

/// Round a byte count up to a power-of-two unit, minimum 1.
fn bytes_to_units(len: usize, unit: usize) -> u8 {
    if len == 0 {
        return 0;
    }
    let units = len.div_ceil(unit);
    // iNES stores the size as a u8. Use the exponential encoding for sizes
    // above 255 units (rare). For typical homebrew, units fits in a u8.
    if units > 0xFF {
        // iNES 2.0 exponential encoding: 0xF0 | exponent.
        // Not used here; clamp to 0xFF.
        0xFF
    } else {
        units as u8
    }
}

// --- .lnx format ----------------------------------------------------------

/// Emit a Lynx .lnx file.
///
/// The 64-byte header is followed by the ROM data. The header fields come
/// from the `#[lnx(...)]` attribute.
fn emit_lnx(obj: &ObjectFile) -> Result<Vec<u8>> {
    let roms = rom_sections(obj);
    let rom_bytes: Vec<u8> = roms.iter().flat_map(|s| s.data.iter().copied()).collect();

    let mut header = [0u8; 64];

    // Magic: "LYNX" at offset 0.
    header[0..4].copy_from_slice(b"LYNX");

    // Version at offset 4 (1 byte). Default to 1.
    header[4] = header_field_u32(obj, "version").unwrap_or(1) as u8;

    // Name at offset 6, 32 bytes. Null-padded.
    if let Some(name) = header_field(obj, "name") {
        let bytes = name.as_bytes();
        let n = bytes.len().min(32);
        header[6..6 + n].copy_from_slice(&bytes[..n]);
    }

    // Manufacturer at offset 38, 16 bytes.
    if let Some(manu) = header_field(obj, "manufacturer") {
        let bytes = manu.as_bytes();
        let n = bytes.len().min(16);
        header[38..38 + n].copy_from_slice(&bytes[..n]);
    }

    // Rotation at offset 54 (1 byte).
    if let Some(rot) = header_field(obj, "rotation") {
        header[54] = match rot {
            "0" | "none" => 0,
            "90" => 1,
            "180" => 2,
            "270" => 3,
            _ => 0,
        };
    }

    // Bank count at offset 48 (1 byte).
    let bank_count = roms.len() as u8;
    header[48] = bank_count;

    // Block size at offset 50 (2 bytes, big-endian). Use 256 (typical).
    let block_size = header_field_u32(obj, "blocksize").unwrap_or(256) as u16;
    header[50] = ((block_size >> 8) & 0xFF) as u8;
    header[51] = (block_size & 0xFF) as u8;

    // Block count at offset 52 (2 bytes, big-endian).
    let block_count = header_field_u32(obj, "blockcount").unwrap_or(0) as u16;
    header[52] = ((block_count >> 8) & 0xFF) as u8;
    header[53] = (block_count & 0xFF) as u8;

    let mut out = Vec::with_capacity(64 + rom_bytes.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&rom_bytes);
    Ok(out)
}

// --- raw format -----------------------------------------------------------

/// Emit a raw binary: concatenated ROM banks in bank order, no header.
fn emit_raw(obj: &ObjectFile) -> Vec<u8> {
    let roms = rom_sections(obj);
    roms.iter().flat_map(|s| s.data.iter().copied()).collect()
}

// --- Intel HEX format -----------------------------------------------------

/// Emit an Intel HEX file as ASCII text.
fn emit_intel_hex(obj: &ObjectFile) -> Result<Vec<u8>> {
    let mut out = String::new();
    let roms = rom_sections(obj);

    for section in &roms {
        let mut addr = section.org;
        let mut data = section.data.as_slice();
        // Emit records of up to 16 bytes.
        while !data.is_empty() {
            let chunk_len = data.len().min(16);
            let chunk = &data[..chunk_len];
            emit_hex_record(&mut out, addr, 0x00, chunk);
            addr += chunk_len as u32;
            data = &data[chunk_len..];
        }
    }

    // End-of-file record.
    emit_hex_record(&mut out, 0x0000, 0x01, &[]);

    Ok(out.into_bytes())
}

/// Write one Intel HEX record.
fn emit_hex_record(out: &mut String, addr: u32, rtype: u8, data: &[u8]) {
    let count = data.len() as u8;
    let addr_lo = (addr & 0xFF) as u8;
    let addr_hi = ((addr >> 8) & 0xFF) as u8;

    let mut sum: u8 = count
        .wrapping_add(addr_hi)
        .wrapping_add(addr_lo)
        .wrapping_add(rtype);
    for &b in data {
        sum = sum.wrapping_add(b);
    }
    let checksum = (0u8).wrapping_sub(sum);

    out.push(':');
    out.push_str(&hex_byte(count));
    out.push_str(&hex_byte(addr_hi));
    out.push_str(&hex_byte(addr_lo));
    out.push_str(&hex_byte(rtype));
    for &b in data {
        out.push_str(&hex_byte(b));
    }
    out.push_str(&hex_byte(checksum));
    out.push('\n');
}

/// Format a byte as two uppercase hex digits.
fn hex_byte(b: u8) -> String {
    format!("{:02X}", b)
}

// --- SEGA Genesis format --------------------------------------------------

/// Emit a SEGA Genesis ROM with the TMSS header at offset 0x100.
fn emit_sega(obj: &ObjectFile) -> Result<Vec<u8>> {
    let roms = rom_sections(obj);
    let mut rom_bytes: Vec<u8> = roms.iter().flat_map(|s| s.data.iter().copied()).collect();

    // The SEGA header lives at 0x100. Ensure the ROM is at least 0x200 bytes.
    if rom_bytes.len() < 0x200 {
        rom_bytes.resize(0x200, 0);
    }

    // "SEGA" magic at 0x100.
    rom_bytes[0x100..0x104].copy_from_slice(b"SEGA");

    // Game name at 0x120, 48 bytes, null-padded.
    if let Some(name) = header_field(obj, "name") {
        let bytes = name.as_bytes();
        let n = bytes.len().min(48);
        rom_bytes[0x120..0x120 + n].copy_from_slice(&bytes[..n]);
    }

    // Region codes at 0x1F0, 3 bytes. Default to "JUE" (Japan/US/Europe).
    let region = header_field(obj, "region").unwrap_or("JUE");
    let region_bytes = region.as_bytes();
    let n = region_bytes.len().min(3);
    rom_bytes[0x1F0..0x1F0 + n].copy_from_slice(&region_bytes[..n]);

    // Checksum at 0x18E (2 bytes, big-endian). Sum of all 16-bit words
    // from 0x200 to the end of the ROM.
    let checksum = sega_checksum(&rom_bytes);
    rom_bytes[0x18E] = ((checksum >> 8) & 0xFF) as u8;
    rom_bytes[0x18F] = (checksum & 0xFF) as u8;

    Ok(rom_bytes)
}

/// Compute the SEGA Genesis ROM checksum.
fn sega_checksum(rom: &[u8]) -> u16 {
    let start = 0x200;
    let mut sum: u16 = 0;
    let mut i = start;
    while i + 1 < rom.len() {
        let word = ((rom[i] as u16) << 8) | (rom[i + 1] as u16);
        sum = sum.wrapping_add(word);
        i += 2;
    }
    // If there is a trailing byte, add it as the high byte of a word.
    if i < rom.len() {
        sum = sum.wrapping_add((rom[i] as u16) << 8);
    }
    sum
}

// --- SNES format ----------------------------------------------------------

/// Emit a SNES ROM with the internal header at 0xFFC0.
///
/// The ROM data already contains the header bytes as part of the image.
/// This function fills in header fields from the `#[snes(...)]` attribute
/// when present, otherwise it returns the ROM data unchanged.
fn emit_snes(obj: &ObjectFile) -> Vec<u8> {
    let roms = rom_sections(obj);
    let mut rom_bytes: Vec<u8> = roms.iter().flat_map(|s| s.data.iter().copied()).collect();

    // The internal header is at 0xFFC0 (HiROM) or 0x7FC0 (LoROM). Use
    // 0xFFC0 by default. Ensure the ROM is large enough.
    let header_addr = 0xFFC0usize;
    if rom_bytes.len() < header_addr + 0x40 {
        rom_bytes.resize(header_addr + 0x40, 0);
    }

    // Game title at 0xFFC0, 21 bytes.
    if let Some(title) = header_field(obj, "title") {
        let bytes = title.as_bytes();
        let n = bytes.len().min(21);
        rom_bytes[header_addr..header_addr + n].copy_from_slice(&bytes[..n]);
    }

    // Map mode at 0xFFD5.
    if let Some(mode) = header_field_u32(obj, "mapmode") {
        rom_bytes[header_addr + 0x15] = mode as u8;
    }

    // ROM type at 0xFFD6.
    if let Some(romtype) = header_field_u32(obj, "romtype") {
        rom_bytes[header_addr + 0x16] = romtype as u8;
    }

    // ROM size at 0xFFD7 (log2 of ROM size in KB).
    if let Some(size) = header_field_u32(obj, "romsize") {
        rom_bytes[header_addr + 0x17] = size as u8;
    } else {
        let kb = rom_bytes.len() / 1024;
        let log2 = (kb as f64).log2().round() as u8;
        rom_bytes[header_addr + 0x17] = log2;
    }

    // RAM size at 0xFFD8.
    if let Some(ramsize) = header_field_u32(obj, "ramsize") {
        rom_bytes[header_addr + 0x18] = ramsize as u8;
    }

    // Region at 0xFFD9.
    if let Some(region) = header_field_u32(obj, "region") {
        rom_bytes[header_addr + 0x19] = region as u8;
    }

    // Vendor at 0xFFDA (2 bytes).
    if let Some(vendor) = header_field_u32(obj, "vendor") {
        rom_bytes[header_addr + 0x1A] = (vendor & 0xFF) as u8;
        rom_bytes[header_addr + 0x1B] = ((vendor >> 8) & 0xFF) as u8;
    }

    // Version at 0xFFDB.
    if let Some(version) = header_field_u32(obj, "version") {
        rom_bytes[header_addr + 0x1B] = version as u8;
    }

    // Checksum at 0xFFDE (2 bytes). Complement sum.
    let (cksum, icksum) = snes_checksum(&rom_bytes);
    rom_bytes[header_addr + 0x1E] = (cksum & 0xFF) as u8;
    rom_bytes[header_addr + 0x1F] = ((cksum >> 8) & 0xFF) as u8;
    rom_bytes[header_addr + 0x1C] = (icksum & 0xFF) as u8;
    rom_bytes[header_addr + 0x1D] = ((icksum >> 8) & 0xFF) as u8;

    rom_bytes
}

/// Compute the SNES checksum and its inverse.
fn snes_checksum(rom: &[u8]) -> (u16, u16) {
    let mut sum: u16 = 0;
    for chunk in rom.chunks_exact(2) {
        let word = (chunk[0] as u16) | ((chunk[1] as u16) << 8);
        sum = sum.wrapping_add(word);
    }
    let icksum = !sum;
    (sum, icksum)
}

// --- Game Boy format ------------------------------------------------------

/// The Nintendo logo bytes for the Game Boy cartridge header (48 bytes).
/// These bytes are required for the ROM to boot on real hardware.
const GB_NINTENDO_LOGO: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

/// Emit a Game Boy / Game Boy Color ROM with the cartridge header at 0x100.
fn emit_gb(obj: &ObjectFile) -> Vec<u8> {
    let roms = rom_sections(obj);
    let mut rom_bytes: Vec<u8> = roms.iter().flat_map(|s| s.data.iter().copied()).collect();

    // Ensure the ROM is large enough to hold the header.
    let header_min = 0x150usize;
    if rom_bytes.len() < header_min {
        rom_bytes.resize(header_min, 0);
    }

    // Nintendo logo at 0x104, 48 bytes.
    rom_bytes[0x104..0x104 + 48].copy_from_slice(&GB_NINTENDO_LOGO);

    // Game title at 0x134, 16 bytes, null-padded.
    if let Some(title) = header_field(obj, "title") {
        let bytes = title.as_bytes();
        let n = bytes.len().min(16);
        rom_bytes[0x134..0x134 + n].copy_from_slice(&bytes[..n]);
    }

    // CGB flag at 0x143. 0x80 = CGB compatible, 0xC0 = CGB only.
    if let Some(cgb) = header_field(obj, "cgb") {
        rom_bytes[0x143] = match cgb {
            "compatible" | "true" => 0x80,
            "only" => 0xC0,
            _ => 0x00,
        };
    }

    // License code at 0x144-0x145 (2 bytes).
    if let Some(license) = header_field(obj, "license") {
        let bytes = license.as_bytes();
        let n = bytes.len().min(2);
        rom_bytes[0x144..0x144 + n].copy_from_slice(&bytes[..n]);
    }

    // Mask ROM version at 0x14C.
    if let Some(version) = header_field_u32(obj, "version") {
        rom_bytes[0x14C] = version as u8;
    }

    // Header checksum at 0x14D.
    let cksum = gb_header_checksum(&rom_bytes);
    rom_bytes[0x14D] = cksum;

    rom_bytes
}

/// Compute the Game Boy header checksum (byte at 0x14D).
fn gb_header_checksum(rom: &[u8]) -> u8 {
    let mut sum: u8 = 0;
    for &b in rom.iter().take(0x14D).skip(0x134) {
        sum = sum.wrapping_sub(b).wrapping_sub(1);
    }
    sum
}

// --- SMS / Game Gear format -----------------------------------------------

/// Emit a Master System / Game Gear ROM with the "TMR SEGA" header at
/// 0x7FF0 when the ROM is large enough.
fn emit_sms(obj: &ObjectFile) -> Vec<u8> {
    let roms = rom_sections(obj);
    let mut rom_bytes: Vec<u8> = roms.iter().flat_map(|s| s.data.iter().copied()).collect();

    // The TMR SEGA header lives at 0x7FF0. Only write it when the ROM is
    // at least 0x8000 bytes.
    if rom_bytes.len() >= 0x8000 {
        rom_bytes[0x7FF0..0x7FF8].copy_from_slice(b"TMR SEGA");

        // Checksum at 0x7FFA (2 bytes, little-endian). Sum of all bytes
        // from 0x0000 to 0x7FEF.
        let cksum = sms_checksum(&rom_bytes, 0x0000, 0x7FF0);
        rom_bytes[0x7FFA] = (cksum & 0xFF) as u8;
        rom_bytes[0x7FFB] = ((cksum >> 8) & 0xFF) as u8;

        // Product code at 0x7FFC (3 bytes, little-endian, first 2 bytes
        // share the low nibble of the region byte).
        if let Some(product) = header_field_u32(obj, "product") {
            let p = product & 0xFFFFF;
            rom_bytes[0x7FFC] = (p & 0xFF) as u8;
            rom_bytes[0x7FFD] = ((p >> 8) & 0xFF) as u8;
            rom_bytes[0x7FFE] = (rom_bytes[0x7FFE] & 0xF0) | ((p >> 16) as u8 & 0x0F);
        }

        // Region and version at 0x7FFF. High nibble = region, low nibble = version.
        let region: u8 = match header_field(obj, "region") {
            Some("japan") => 0x0,
            Some("us") => 0x4,
            Some("europe") => 0x8,
            _ => 0x4,
        };
        let version = (header_field_u32(obj, "version").unwrap_or(0) & 0x0F) as u8;
        rom_bytes[0x7FFF] = (region << 4) | version;
    }

    rom_bytes
}

/// Compute a simple byte-sum checksum over a range.
fn sms_checksum(rom: &[u8], start: usize, end: usize) -> u16 {
    let mut sum: u16 = 0;
    for &b in rom.get(start..end).unwrap_or(&[]) {
        sum = sum.wrapping_add(b as u16);
    }
    sum
}

// --- Atari 7800 format ----------------------------------------------------

/// Emit an Atari 7800 ROM with the 78-byte "ATARI7800" header.
fn emit_a78(obj: &ObjectFile) -> Result<Vec<u8>> {
    let roms = rom_sections(obj);
    let rom_bytes: Vec<u8> = roms.iter().flat_map(|s| s.data.iter().copied()).collect();

    let mut header = [0u8; 78];

    // Magic: "ATARI7800" at offset 0, 9 bytes.
    header[0..9].copy_from_slice(b"ATARI7800");

    // Cart name at offset 17, 32 bytes, null-padded.
    if let Some(name) = header_field(obj, "name") {
        let bytes = name.as_bytes();
        let n = bytes.len().min(32);
        header[17..17 + n].copy_from_slice(&bytes[..n]);
    }

    // Mapper at offset 49 (1 byte).
    if let Some(mapper) = header_field_u32(obj, "mapper") {
        header[49] = mapper as u8;
    }

    // Region at offset 50 (1 byte). 0 = NTSC, 1 = PAL.
    if let Some(region) = header_field(obj, "region") {
        header[50] = match region {
            "pal" => 1,
            _ => 0,
        };
    }

    // Pokey flags at offset 53 (1 byte). Bit 0 = Pokey present.
    if header_field_bool(obj, "pokey") {
        header[53] |= 0x01;
    }

    let mut out = Vec::with_capacity(78 + rom_bytes.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&rom_bytes);
    Ok(out)
}

// --- Tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use op_ir::{HeaderFields, ObjectFile, Section, SectionKind};

    fn make_obj(target: &str, sections: Vec<Section>) -> ObjectFile {
        ObjectFile::new(target).with_sections(sections)
    }

    /// Build a small ROM section for tests.
    fn rom_section(bank: u32, org: u32, data: Vec<u8>) -> Section {
        Section {
            name: format!("rom_bank{}", bank),
            kind: SectionKind::Rom,
            org,
            bank,
            maxsize: data.len() as u32,
            symbols: Vec::new(),
            relocations: Vec::new(),
            data,
        }
    }

    #[test]
    fn raw_format_concatenates_rom_banks() {
        let obj = make_obj(
            "rp2A03-nintendo-nes-ntsc",
            vec![
                rom_section(0, 0xC000, vec![0xAA; 4]),
                rom_section(1, 0x0000, vec![0xBB; 4]),
            ],
        );
        let bytes = emit_linked(&obj, "raw").unwrap();
        assert_eq!(bytes, vec![0xAA, 0xAA, 0xAA, 0xAA, 0xBB, 0xBB, 0xBB, 0xBB]);
    }

    #[test]
    fn ines_format_writes_magic_header() {
        let obj = make_obj(
            "rp2A03-nintendo-nes-ntsc",
            vec![
                rom_section(0, 0xC000, vec![0x00; 16 * 1024]),
                Section {
                    name: "chr_bank0".into(),
                    kind: SectionKind::Chr,
                    org: 0,
                    bank: 0,
                    maxsize: 8 * 1024,
                    symbols: Vec::new(),
                    relocations: Vec::new(),
                    data: vec![0x11; 8 * 1024],
                },
            ],
        );
        let bytes = emit_linked(&obj, "ines").unwrap();
        // Magic: N E S 0x1A.
        assert_eq!(&bytes[0..4], &[b'N', b'E', b'S', 0x1A]);
        // PRG size: 1 unit (16KB).
        assert_eq!(bytes[4], 1);
        // CHR size: 1 unit (8KB).
        assert_eq!(bytes[5], 1);
        // Total size: 16 + 16KB + 8KB.
        assert_eq!(bytes.len(), 16 + 16 * 1024 + 8 * 1024);
    }

    #[test]
    fn ines_format_records_mapper_and_flags() {
        let mut obj = make_obj(
            "rp2A03-nintendo-nes-ntsc",
            vec![rom_section(0, 0xC000, vec![0x00; 16 * 1024])],
        );
        obj.header = Some(HeaderFields {
            format: "ines".into(),
            fields: vec![
                ("mapper".into(), "7".into()),
                ("mirroring".into(), "horizontal".into()),
                ("battery".into(), "true".into()),
                ("trainer".into(), "false".into()),
                ("fourscreen".into(), "false".into()),
            ],
        });
        let bytes = emit_linked(&obj, "ines").unwrap();
        // Flags 6: mapper_lo=7 << 4 | mirroring bit 0 (1) | battery (2) = 0x73.
        assert_eq!(bytes[6], 0x73);
        // Flags 7: mapper_hi = 0.
        assert_eq!(bytes[7], 0x00);
    }

    #[test]
    fn lnx_format_writes_lynx_magic() {
        let obj = make_obj(
            "65c02-atari-lynx-ntsc",
            vec![rom_section(0, 0x0000, vec![0xFF; 256])],
        );
        let bytes = emit_linked(&obj, "lnx").unwrap();
        assert_eq!(&bytes[0..4], b"LYNX");
        assert_eq!(bytes.len(), 64 + 256);
    }

    #[test]
    fn intel_hex_writes_data_and_eof_records() {
        let obj = make_obj(
            "mos6502-none-none-ntsc",
            vec![rom_section(0, 0x8000, vec![0x01, 0x02, 0x03])],
        );
        let bytes = emit_linked(&obj, "hex").unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        // First record: :03 8000 00 010203 FA
        assert!(text.starts_with(":03800000010203"));
        // End-of-file record on its own line.
        assert!(text.contains(":00000001FF"));
    }

    #[test]
    fn sega_format_writes_sega_magic_at_0x100() {
        let obj = make_obj(
            "mos6502-sega-genesis-ntsc",
            vec![rom_section(0, 0x0000, vec![0x00; 0x400])],
        );
        let bytes = emit_linked(&obj, "sega").unwrap();
        assert_eq!(&bytes[0x100..0x104], b"SEGA");
    }

    #[test]
    fn default_format_for_nes_is_ines() {
        assert_eq!(
            default_format_for_target("rp2A03-nintendo-nes-ntsc"),
            "ines"
        );
    }

    #[test]
    fn default_format_for_lynx_is_lnx() {
        assert_eq!(default_format_for_target("65c02-atari-lynx-ntsc"), "lnx");
    }

    #[test]
    fn default_format_for_unknown_is_raw() {
        assert_eq!(default_format_for_target("mos6502-none-none-ntsc"), "raw");
    }

    /// Helper trait to construct an ObjectFile with sections inline.
    trait ObjectFileExt: Sized {
        fn with_sections(self, sections: Vec<Section>) -> Self;
    }

    impl ObjectFileExt for ObjectFile {
        fn with_sections(mut self, sections: Vec<Section>) -> Self {
            self.sections = sections;
            self
        }
    }
}
