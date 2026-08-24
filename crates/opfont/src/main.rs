//! opfont — TTF to Op font cart rasterizer.
//!
//! This tool loads a .ttf font file, rasterizes the glyphs to 8x8
//! one-bit-per-pixel tiles, and generates an Op library cart that
//! contains the font data in the format used by the std library font
//! system.

use anyhow::{anyhow, Result};
use clap::Parser;
use dialoguer::{theme::ColorfulTheme, MultiSelect, Select};
use fontdue::Font;
use fontdue::FontSettings;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

// --- CLI --------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "opfont", version, about)]
struct Args {
    /// Output directory for the generated font cart. Defaults to the
    /// current directory.
    #[arg(short = 'o', long = "output", default_value = ".")]
    output: String,

    /// Font name override. Defaults to the TTF file name stem.
    #[arg(short = 'n', long = "name")]
    name: Option<String>,

    /// Skip the interactive wizard and use default options.
    #[arg(long = "no-interactive")]
    no_interactive: bool,
}

// --- Target triples --------------------------------------------------------

const TARGETS: &[(&str, &str)] = &[
    (
        "rp2A03-nintendo-nes-ntsc",
        "NES NTSC (8x8 tiles, 1bpp stored, 2-plane CHR)",
    ),
    (
        "sm83-nintendo-gameboy",
        "DMG Game Boy (8x8 tiles, 1bpp stored, 2bpp interleaved)",
    ),
    (
        "sm83-nintendo-gameboy-color",
        "GBC Game Boy Color (8x8 tiles, 1bpp stored, 2bpp interleaved)",
    ),
];

const TARGET_SUFFIXES: &[(&str, &str)] = &[
    ("rp2A03-nintendo-nes-ntsc", "NES"),
    ("sm83-nintendo-gameboy", "GB"),
    ("sm83-nintendo-gameboy-color", "GBC"),
];

// --- Character sets --------------------------------------------------------

const CHARSET_UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const CHARSET_LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const CHARSET_DIGITS: &str = "0123456789";
const CHARSET_SYMBOLS: &str = " !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";
#[allow(dead_code)]
const CHARSET_CONTROL: &str = "";

// --- Font sizes / styles / spacing ----------------------------------------

const SIZES: &[(&str, u8)] = &[("TINY", 6), ("SMALL", 8), ("LARGE", 8), ("EXTRA_LARGE", 16)];

const STYLES: &[&str] = &["NORMAL", "BOLD", "ITALIC", "BOLD_ITALIC"];
const SPACINGS: &[&str] = &["MONOSPACE", "PROPORTIONAL"];

// --- Main ------------------------------------------------------------------

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Find .ttf files in the current directory.
    let ttf_files: Vec<String> = glob::glob("*.ttf")?
        .filter_map(|e| e.ok())
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    if ttf_files.is_empty() {
        return Err(anyhow!("No .ttf files found in the current directory."));
    }

    // 2. Select the TTF file.
    let ttf_idx = if args.no_interactive {
        0
    } else {
        Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a .ttf font file")
            .items(&ttf_files)
            .default(0)
            .interact()?
    };
    let ttf_path = &ttf_files[ttf_idx];
    let font_name = args.name.clone().unwrap_or_else(|| {
        PathBuf::from(ttf_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string().to_uppercase())
            .unwrap_or_else(|| "FONT".to_string())
    });

    // 3. Select target triples.
    let target_indices: Vec<usize> = if args.no_interactive {
        vec![0, 1, 2]
    } else {
        let target_labels: Vec<String> =
            TARGETS.iter().map(|(t, d)| format!("{t} — {d}")).collect();
        MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select target triples (space to toggle)")
            .items(&target_labels)
            .interact()?
    };
    let selected_targets: Vec<&str> = target_indices.iter().map(|&i| TARGETS[i].0).collect();
    if selected_targets.is_empty() {
        return Err(anyhow!("No target triples selected."));
    }

    // 4. Select variants.
    let size_indices: Vec<usize> = if args.no_interactive {
        vec![1] // SMALL only
    } else {
        let size_labels: Vec<String> = SIZES
            .iter()
            .map(|(n, h)| format!("{n} ({h}x{h})"))
            .collect();
        MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select font sizes (space to toggle)")
            .items(&size_labels)
            .interact()?
    };
    let style_indices: Vec<usize> = if args.no_interactive {
        vec![0] // NORMAL only
    } else {
        MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select font styles (space to toggle)")
            .items(STYLES)
            .interact()?
    };
    let spacing_idx: usize = if args.no_interactive {
        0
    } else {
        Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select spacing")
            .items(SPACINGS)
            .default(0)
            .interact()?
    };

    // 5. Select character sets.
    let charset_labels = [
        "Uppercase (A-Z)",
        "Lowercase (a-z)",
        "Numbers (0-9)",
        "Symbols/punctuation",
        "Control (0x00-0x1F)",
    ];
    let charset_indices: Vec<usize> = if args.no_interactive {
        vec![0, 1, 2, 3] // upper, lower, digits, symbols
    } else {
        MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select character sets (space to toggle)")
            .items(&charset_labels)
            .interact()?
    };

    // 6. Build the character set.
    let mut chars: BTreeSet<u8> = BTreeSet::new();
    if charset_indices.contains(&0) {
        chars.extend(CHARSET_UPPER.bytes());
    }
    if charset_indices.contains(&1) {
        chars.extend(CHARSET_LOWER.bytes());
    }
    if charset_indices.contains(&2) {
        chars.extend(CHARSET_DIGITS.bytes());
    }
    if charset_indices.contains(&3) {
        chars.extend(CHARSET_SYMBOLS.bytes());
    }
    if charset_indices.contains(&4) {
        for c in 0x00..=0x1Fu8 {
            chars.insert(c);
        }
    }
    chars.insert(b' '); // Always include space.
    let chars: Vec<u8> = chars.into_iter().collect();
    if chars.is_empty() {
        return Err(anyhow!("No characters selected."));
    }

    // 7. Read the TTF file.
    let font_data = fs::read(ttf_path)?;
    let font = Font::from_bytes(font_data, FontSettings::default())
        .map_err(|e| anyhow!("fontdue error: {}", e))?;

    // 8. Rasterize glyphs for each size+style combo.
    let selected_sizes: Vec<(&str, u8)> = size_indices.iter().map(|&i| SIZES[i]).collect();
    let selected_styles: Vec<&str> = style_indices.iter().map(|&i| STYLES[i]).collect();
    let spacing = SPACINGS[spacing_idx];

    // Build all variant data blobs.
    let mut variants: Vec<VariantData> = Vec::new();
    for &size in &selected_sizes {
        for &style in &selected_styles {
            let blob = rasterize_font(&font, &chars, size.1, style, &font_name);
            if blob.tiles > 0 {
                variants.push(VariantData {
                    size: size.0,
                    style,
                    blob,
                });
            }
        }
    }

    if variants.is_empty() {
        return Err(anyhow!("No variants could be rasterized from the font."));
    }

    // 9. Generate the font cart.
    let cart_name = format!("{}-font", font_name.to_lowercase());
    let output_dir = PathBuf::from(&args.output);

    generate_font_cart(
        &output_dir,
        &cart_name,
        &font_name,
        &variants,
        &selected_targets,
        spacing,
    )?;

    println!(
        "Font cart '{}' generated in {}",
        cart_name,
        output_dir.display()
    );
    println!("Install it with: cart install {}", cart_name);

    Ok(())
}

// --- Rasterizer ------------------------------------------------------------

struct FontBlob {
    /// The flat byte array: [flags][tile_count][encoding_table][1bpp_tiles]
    data: Vec<u8>,
    /// Number of tiles.
    tiles: usize,
    /// Tile height (8 or 16).
    tile_h: u8,
    /// Encoding mode (0=256, 1=128, 2=none).
    encoding: u8,
}

struct VariantData {
    size: &'static str,
    style: &'static str,
    blob: FontBlob,
}

fn rasterize_font(
    font: &Font,
    chars: &[u8],
    pixel_size: u8,
    style: &str,
    _font_name: &str,
) -> FontBlob {
    // Determine which glyph faces are available.
    // fontdue doesn't expose face flags directly; we rasterize all
    // requested chars and skip if the glyph is empty.

    let tile_h = if pixel_size > 8 { 16 } else { 8 };
    let tile_w = 8u8;

    // Rasterize each character to a 1bpp tile.
    let mut tiles: Vec<Vec<u8>> = Vec::new();
    let mut char_to_tile: Vec<u8> = vec![0u8; 256]; // encoding table
    let mut tile_idx: usize = 1; // tile 0 is always the space/blank

    // Tile 0: blank tile (all zeros).
    tiles.push(vec![0u8; tile_h as usize]);

    for &c in chars {
        if c == b' ' {
            char_to_tile[c as usize] = 0;
            continue;
        }

        let (metrics, bitmap) = font.rasterize(c as char, pixel_size as f32);

        // Extract the 8x8 (or 8x16) tile from the bitmap.
        let tile = extract_tile(
            &bitmap,
            metrics.width,
            metrics.height,
            tile_w,
            tile_h,
            style,
        );

        if tile.iter().all(|&b| b == 0) {
            // Empty glyph — map to space.
            char_to_tile[c as usize] = 0;
            continue;
        }

        char_to_tile[c as usize] = tile_idx as u8;
        tiles.push(tile);
        tile_idx += 1;
    }

    // Build the encoding table.
    // Determine encoding mode: if all chars < 128, use ENCODE_128.
    // If all chars < 256, use ENCODE_256. Otherwise ENCODE_NONE.
    let max_char = chars.iter().copied().max().unwrap_or(0);
    let enc_mode = if max_char < 128 {
        1u8
    } else if max_char < 255 {
        0u8
    } else {
        2u8
    };
    let enc_table_size = match enc_mode {
        0 => 256,
        1 => 128,
        _ => 0,
    };

    // Build the flat byte array.
    let flags = enc_mode | 4; // 4 = compressed (1bpp)
    let tile_count = tiles.len() as u8;
    let mut data = Vec::new();

    data.push(flags);
    data.push(tile_count);

    // Encoding table.
    if enc_mode == 0 || enc_mode == 1 {
        data.extend_from_slice(&char_to_tile[..enc_table_size]);
    }

    // Tile data.
    for tile in &tiles {
        data.extend_from_slice(tile);
    }

    FontBlob {
        data,
        tiles: tiles.len(),
        tile_h,
        encoding: enc_mode,
    }
}

fn extract_tile(
    bitmap: &[u8],
    bm_width: usize,
    bm_height: usize,
    tile_w: u8,
    tile_h: u8,
    style: &str,
) -> Vec<u8> {
    let tw = tile_w as usize;
    let th = tile_h as usize;
    let mut tile = vec![0u8; th];

    // Copy the bitmap into the tile, centered.
    for y in 0..bm_height.min(th) {
        let mut row: u8 = 0;
        for x in 0..bm_width.min(tw) {
            let coverage = bitmap[y * bm_width + x];
            if coverage > 127 {
                row |= 1 << (7 - x);
            }
        }
        tile[y] = row;
    }

    // Apply style transforms.
    match style {
        "ITALIC" | "BOLD_ITALIC" => {
            // Shear: shift each row right by floor(y * 0.3).
            let mut shifted = vec![0u8; th];
            for y in 0..th {
                let shift = (y as f32 * 0.3) as usize;
                if shift < 8 {
                    shifted[y] = tile[y] >> shift;
                }
            }
            tile = shifted;
        }
        _ => {}
    }

    match style {
        "BOLD" | "BOLD_ITALIC" => {
            // Dilate: OR each row with itself shifted right by 1.
            for row in tile.iter_mut().take(th) {
                *row |= *row >> 1;
            }
        }
        _ => {}
    }

    tile
}

// --- Cart generation -------------------------------------------------------

fn generate_font_cart(
    output_dir: &PathBuf,
    cart_name: &str,
    font_name: &str,
    variants: &[VariantData],
    targets: &[&str],
    spacing: &str,
) -> Result<()> {
    let cart_dir = output_dir.join(cart_name);

    // Run `cart init --lib <name> --target <first_target>` to create the cart skeleton.
    let status = Command::new("cart")
        .arg("init")
        .arg("--lib")
        .arg(cart_name)
        .arg("--target")
        .arg(targets[0])
        .current_dir(output_dir)
        .status()?;

    if !status.success() {
        return Err(anyhow!("cart init --lib {} failed", cart_name));
    }

    // Write src/lib.op.
    let lib_op = "//! Generated font library.\n\npub mod font;\n";
    fs::write(cart_dir.join("src/lib.op"), lib_op)?;

    // Create src/font/ directory.
    let font_dir = cart_dir.join("src/font");
    fs::create_dir_all(&font_dir)?;

    // Write src/font/mod.op with font_t constants.
    let mut mod_op = String::new();
    mod_op.push_str("//! Font constants for this library.\n\n");
    mod_op.push_str("use std::font::font_t;\n\n");

    // Write data files and mod.op constants for each target.
    let data_dir = font_dir.join("data");
    fs::create_dir_all(&data_dir)?;

    for target in targets {
        let suffix = target_suffix(target);
        for variant in variants {
            let array_name = format!(
                "{}_{}_{}_{}",
                font_name, variant.size, variant.style, suffix
            );
            let const_name = format!("FONT_{}_{}_{}", font_name, variant.size, variant.style);

            // Write the data array file.
            let data_file = format!("{}.op", array_name.to_lowercase());
            let mut data_op = String::new();
            data_op.push_str(&format!(
                "//! Font data for {} {} {} on {}.\n\n",
                font_name, variant.size, variant.style, target
            ));
            data_op.push_str(&format!(
                "#[cfg(machine = \"{}\")]\n",
                if *target == "sm83-nintendo-gameboy" || *target == "sm83-nintendo-gameboy-color" {
                    "gameboy"
                } else {
                    "nes"
                }
            ));
            data_op.push_str(&format!(
                "const {}: [u8; {}] = [",
                array_name,
                variant.blob.data.len()
            ));
            for (i, byte) in variant.blob.data.iter().enumerate() {
                if i % 16 == 0 {
                    data_op.push_str("\n    ");
                }
                data_op.push_str(&format!("0x{:02x},", byte));
            }
            data_op.push_str("\n];\n");
            fs::write(data_dir.join(&data_file), data_op)?;

            // Write the font_t constant in mod.op.
            let cfg_target =
                if *target == "sm83-nintendo-gameboy" || *target == "sm83-nintendo-gameboy-color" {
                    "gameboy"
                } else {
                    "nes"
                };
            let cfg_variant = if *target == "sm83-nintendo-gameboy-color" {
                "color"
            } else {
                ""
            };

            if cfg_variant.is_empty() {
                mod_op.push_str(&format!("#[cfg(machine = \"{}\")]\n", cfg_target));
            } else {
                mod_op.push_str(&format!(
                    "#[cfg(all(machine = \"{}\", variant = \"{}\"))]\n",
                    cfg_target, cfg_variant
                ));
            }
            mod_op.push_str(&format!("const {}: font_t = font_t {{\n", const_name));
            mod_op.push_str(&format!("    name: \"{}\",\n", font_name.to_lowercase()));
            mod_op.push_str(&format!(
                "    tile_w: 8, tile_h: {},\n",
                variant.blob.tile_h
            ));
            mod_op.push_str(&format!(
                "    encoding: {}, tile_count: {}, flags: {},\n",
                variant.blob.encoding,
                variant.blob.tiles,
                variant.blob.encoding | 4
            ));
            mod_op.push_str(&format!(
                "    size: {}, style: {}, spacing: {},\n",
                variant.size, variant.style, spacing
            ));
            mod_op.push_str(&format!("    data: {},\n", array_name));
            mod_op.push_str("};\n\n");
        }
    }

    fs::write(font_dir.join("mod.op"), mod_op)?;

    // Update Cart.toml to add std dependency.
    let cart_toml_path = cart_dir.join("Cart.toml");
    let cart_toml = fs::read_to_string(&cart_toml_path)?;
    let updated_toml = if cart_toml.contains("[dependencies]") {
        format!(
            "{}\nstd = {{ version = \"0.5.0\", git = \"https://github.com/op-language/std\" }}\n",
            cart_toml
        )
    } else {
        format!("{}\n[dependencies]\nstd = {{ version = \"0.5.0\", git = \"https://github.com/op-language/std\" }}\n", cart_toml)
    };
    fs::write(cart_toml_path, updated_toml)?;

    Ok(())
}

fn target_suffix(target: &str) -> &'static str {
    for (t, s) in TARGET_SUFFIXES {
        if *t == target {
            return s;
        }
    }
    "UNKNOWN"
}
