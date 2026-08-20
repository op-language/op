# Supported Emulators

This document lists the best Linux emulator for each target system that
the Op compiler supports. For each system, the document gives the ROM
file format, the install command, and the debug-target configuration.

## Quick reference: ROM format per system

| System | ROM format | Extension | Header |
|--------|-----------|-----------|--------|
| NES (NTSC/PAL) | iNES | `.nes` | 16-byte `NES\x1A` header |
| Atari Lynx | Handy .lnx | `.lnx` | 64-byte LYNX header |
| Apple II/IIc/IIe/IIe Enhanced | Disk image | `.dsk`/`.po` | Raw sector image, no header |
| Apple IIgs | 2IMG container | `.2mg` | 64-byte 2IMG header |
| Atari 800 (NTSC/PAL) | ATR/XEX | `.atr`/`.xex` | ATR 16-byte header; XEX load-address header |
| Atari 2600 | Raw binary | `.bin`/`.a26` | No header (bankswitch auto-detected) |
| Atari 5200 | Raw binary | `.bin`/`.a52` | No header; requires `5200.rom` BIOS |
| Atari 7800 | A78 | `.a78` | 78-byte `ATARI7800` header |
| Commodore 64 | PRG/D64/CRT | `.prg`/`.d64`/`.crt` | PRG 2-byte load address; CRT 16-byte header |
| NEC PC Engine | Raw binary | `.pce` | No header for cart images |
| Neo Geo AES | MAME zip / .neo | `.zip`/`.neo` | MAME ROM set; .neo has 5-row header |
| Sega Genesis | SEGA binary | `.bin`/`.md` | 256-byte SEGA TMSS header at offset 0x100 |
| SNES | SFC/SMC | `.sfc`/`.smc` | Internal header at 0xFFC0; .smc adds 512-byte copier header |
| Game Boy | GB | `.gb` | Cartridge header at 0x100 (Nintendo logo) |
| Game Boy Color | GBC | `.gbc` | Cartridge header at 0x100 with CGB flag |
| Sega Game Gear | GG | `.gg` | TMR SEGA header at 0x7FF0 (optional) |
| Sega Master System | SMS | `.sms` | TMR SEGA header at 0x7FF0 |
| Sega SG-1000 | Raw binary | `.sg` | No header |
| Sinclair ZX80 | P/ROM | `.p`/`.rom` | Raw RAM dump or ROM image |
| Sinclair ZX81 | P81/ROM | `.p81`/`.rom` | P81 has 17-byte header; ROM is raw |
| Sinclair Spectrum | Z80/SNA/TAP | `.z80`/`.sna`/`.tap` | Z80 snapshot header (30/55+ bytes) |
| Texas Instruments TI-85 | ROM | `.rom` | Raw ROM dump, no header |

## Quick reference: best native Linux debug target per system

| System | Best debug target | Launch command |
|--------|------------------|----------------|
| NES | Mesen2 or FCEUX | `mesen2 game.nes` or `fceux game.nes` |
| Atari Lynx | Mednafen | `mednafen game.lnx` |
| Apple II family | MAME | `mame apple2e -flop1 game.dsk -debug` |
| Apple IIgs | MAME | `mame apple2gs -flop1 game.2mg -debug` |
| Atari 800 | MAME | `mame a800 -flop1 game.atr -debug` |
| Atari 2600 | Stella | `stella -debug game.bin` |
| Atari 5200 | MAME | `mame a5200 -cart game.bin -debug` |
| Atari 7800 | MAME | `mame a7800 -cart game.a78 -debug` |
| Commodore 64 | VICE | `x64sc -binarymonitor game.prg` |
| NEC PC Engine | Mednafen | `mednafen game.pce` |
| Neo Geo AES | MAME | `mame aes -cart game.zip -debug` |
| Sega Genesis | BlastEm | `blastem game.md` |
| SNES | bsnes or Mesen2 | `bsnes game.sfc` |
| Game Boy/Game Boy Color | SameBoy or mGBA | `sameboy game.gb` or `mgba-sdl -g 1234 game.gb` |
| Sega Game Gear | MAME | `mame gamegear -cart game.gg -debug` |
| Sega Master System | MAME | `mame sms -cart game.sms -debug` |
| Sega SG-1000 | MAME | `mame sg1000 -cart game.sg -debug` |
| Sinclair ZX80 | MAME | `mame zx80 -quik game.p -debug` |
| Sinclair ZX81 | SZ81 or MAME | `sz81 game.p81` or `mame zx81 -debug` |
| Sinclair Spectrum | Fuse or MAME | `fuse game.z80` or `mame spectrum -debug` |
| TI-85 | TilEm2 | `tilem85 -rom ti85.rom` |

## Detailed emulator information

### Nintendo Entertainment System (NES)

- **Emulators**: Mesen2 (https://github.com/SourMesen/Mesen2), FCEUX
  (https://fceux.com), Mednafen (https://mednafen.github.io/)
- **ROM format**: iNES `.nes` with 16-byte `NES\x1A` header. Mapper,
  PRG/CHR sizes, mirroring, and region bits are in the header.
- **Install**: `apt install fceux` or `apt install mednafen`. Mesen2:
  build from source.
- **Debug target**: Mesen2 has a built-in debugger with a scriptable
  interface. FCEUX has a debugger and Lua scripting. Mednafen is
  CLI-driven and deterministic.
- **Required BIOS**: None for cart-based games.

### Atari Lynx

- **Emulators**: Mednafen (Lynx core), Handy (via RetroArch)
- **ROM format**: `.lnx` with 64-byte Handy header. Magic bytes `LYNX`
  at offset 0.
- **Install**: `apt install mednafen`.
- **Debug target**: Mednafen is CLI-driven. Use the built-in debugger.
- **Required BIOS**: `lynxboot.img` (512-byte Lynx boot ROM). Place in
  the Mednafen firmware directory.

### Apple II / IIc / IIe / IIe Enhanced

- **Emulators**: MAME (https://www.mamedev.org/), AppleWin (via Wine)
- **ROM format**: Disk images `.dsk`, `.po`, `.do` (140KB 5.25" images).
  No cartridge ROM format.
- **Install**: `apt install mame`.
- **Debug target**: MAME with `-debug` flag. Launch:
  `mame apple2e -flop1 game.dsk -debug`. MAME has a built-in debugger
  and Lua memory engine.
- **Required BIOS**: Apple II/II+ ROM, IIe ROM, IIc ROM. Place in the
  MAME ROMs directory.

### Apple IIgs

- **Emulators**: GSplus (https://github.com/digarok/GSplus), MAME
- **ROM format**: `.2mg` (2IMG container with 64-byte header) or `.dsk`.
- **Install**: `apt install mame`. GSplus: build from source.
- **Debug target**: MAME with `-debug` flag. Launch:
  `mame apple2gs -flop1 game.2mg -debug`.
- **Required BIOS**: Apple IIgs ROM01 or ROM03. Place in the MAME ROMs
  directory.

### Atari 800 (NTSC/PAL)

- **Emulators**: Atari800 (https://atari800.github.io/), MAME
- **ROM format**: `.atr` (16-byte header) for disk images. `.xex` for
  executables (load-address header).
- **Install**: `apt install atari800` or `apt install mame`.
- **Debug target**: MAME with `-debug` flag. Launch:
  `mame a800 -flop1 game.atr -debug`.
- **Required BIOS**: `ATARIOSA.ROM`, `ATARIOSB.ROM`, `ATARIXL.ROM`,
  `ATARIBAS.ROM`. Place in the emulator ROM directory.

### Atari 2600

- **Emulators**: Stella (https://stella-emu.github.io/)
- **ROM format**: Raw `.bin` or `.a26`. No header. Bankswitch scheme is
  auto-detected by Stella.
- **Install**: `apt install stella`.
- **Debug target**: Stella has a first-class built-in debugger with TIA,
  CPU, RAM views, breakpoints, and trace. Launch:
  `stella -debug game.bin`.
- **Required BIOS**: None.

### Atari 5200

- **Emulators**: Atari800, MAME
- **ROM format**: Raw `.bin` or `.a52`. No header.
- **Install**: `apt install atari800` or `apt install mame`.
- **Debug target**: MAME with `-debug` flag. Launch:
  `mame a5200 -cart game.bin -debug`.
- **Required BIOS**: `5200.rom` BIOS. Place in the emulator ROM
  directory.

### Atari 7800

- **Emulators**: A7800 (https://github.com/7800-dev/a7800), MAME
- **ROM format**: `.a78` with 78-byte `ATARI7800` header. The header
  encodes cart name, mapper, region, and Pokey flags.
- **Install**: `apt install mame`. A7800: build from source.
- **Debug target**: MAME with `-debug` flag. Launch:
  `mame a7800 -cart game.a78 -debug`.
- **Required BIOS**: `7800 BIOS (NTSC).a78` or PAL BIOS. Place in the
  emulator ROM directory.

### Commodore 64

- **Emulators**: VICE (https://vice-emu.sourceforge.io/)
- **ROM format**: `.prg` (2-byte load address + bytes), `.d64` (GCR disk
  image), `.crt` (cartridge with 16-byte CRT header).
- **Install**: `apt install vice`.
- **Debug target**: VICE has a built-in monitor accessible via a TCP
  binary monitor protocol. Launch:
  `x64sc -binarymonitor -moncommands init.vs game.prg`. Connect a
  debugger to the binary monitor port.
- **Required BIOS**: `basic`, `kernal`, `chargen` ROMs. VICE ships with
  OpenROMs and accepts originals.

### NEC PC Engine

- **Emulators**: Mednafen (PCE core), Beetle PCE (libretro core)
- **ROM format**: Raw `.pce` cartridge images. No header. ROM size
  determines banking.
- **Install**: `apt install mednafen`.
- **Debug target**: Mednafen is CLI-driven. Use the built-in debugger.
  Launch: `mednafen game.pce`.
- **Required BIOS**: `syscard3.pce` for CD games. Not needed for cart
  games.

### Neo Geo AES

- **Emulators**: MAME, FinalBurn Neo (https://github.com/finalburnneo/FBNeo)
- **ROM format**: MAME zip sets (e.g. `mslug.zip`) with individual
  V/C/M/P/S1 ROM files. Requires `neogeo.zip` BIOS. FBNeo also accepts
  `.neo` single-file format.
- **Install**: `apt install mame`.
- **Debug target**: MAME with `-debug` flag. Launch:
  `mame aes -cart mslug -debug`.
- **Required BIOS**: `neogeo.zip` containing `000-lo.lo`, `sfix.sfix`,
  `sm1.sm1`, and BIOS ROMs. Place in the MAME ROMs directory.

### Sega Genesis (Mega Drive)

- **Emulators**: BlastEm (https://github.com/rmrobinson/blastem),
  Genesis Plus GX (libretro core), Mednafen
- **ROM format**: `.bin` or `.md` with 256-byte SEGA TMSS header at
  offset 0x100. The header has game name, region codes, and checksum.
- **Install**: BlastEm: build from source. Mednafen: `apt install
  mednafen`.
- **Debug target**: BlastEm has a built-in 68000/Z80 debugger. Launch:
  `blastem game.md`.
- **Required BIOS**: None for cart games.

### Super Nintendo Entertainment System (SNES)

- **Emulators**: bsnes (https://github.com/bsnes-emu/bsnes), Snes9x
  (https://github.com/snes9xgit/snes9x), Mesen2
- **ROM format**: `.sfc` (raw ROM with internal SNES header at 0xFFC0)
  or `.smc` (adds 512-byte SMC copier header, auto-stripped by
  emulators).
- **Install**: `apt install bsnes` or `apt install snes9x`.
- **Debug target**: bsnes has a built-in debugger with CPU/S-PPU/SMP/DSP
  views. Mesen2 has a scriptable SNES debugger.
- **Required BIOS**: None for cart games.

### Nintendo Game Boy / Game Boy Color

- **Emulators**: SameBoy (https://github.com/LIJI32/SameBoy), mGBA
  (https://github.com/mgba-emu/mgba)
- **ROM format**: `.gb` (Game Boy) and `.gbc` (Game Boy Color) with
  cartridge header at offset 0x100 (Nintendo logo, game title, CGB
  flag).
- **Install**: `apt install sameboy` or `apt install mgba-sdl`.
- **Debug target**: SameBoy has a built-in debugger with a GDB stub
  interface. mGBA-SDL supports a GDB stub: `mgba-sdl -g 1234 game.gb`.
- **Required BIOS**: None. The cartridge header is the format.

### Sega Game Gear

- **Emulators**: Genesis Plus GX (libretro core), Mednafen, MAME
- **ROM format**: `.gg` raw binary. TMR SEGA header at 0x7FF0 when
  present.
- **Install**: `apt install mednafen` or `apt install mame`.
- **Debug target**: MAME with `-debug` flag. Launch:
  `mame gamegear -cart game.gg -debug`.
- **Required BIOS**: `bios.gg` (optional).

### Sega Master System

- **Emulators**: Genesis Plus GX (libretro core), Mednafen, MAME
- **ROM format**: `.sms` raw binary with TMR SEGA header at 0x7FF0.
- **Install**: `apt install mednafen` or `apt install mame`.
- **Debug target**: MAME with `-debug` flag. Launch:
  `mame sms -cart game.sms -debug`.
- **Required BIOS**: `bios_E.sms`, `bios_U.sms`, `bios_J.sms`
  (optional but needed for some games).

### Sega SG-1000

- **Emulators**: Genesis Plus GX (libretro core), Mednafen, MAME
- **ROM format**: `.sg` raw binary. No header.
- **Install**: `apt install mednafen` or `apt install mame`.
- **Debug target**: MAME with `-debug` flag. Launch:
  `mame sg1000 -cart game.sg -debug`.
- **Required BIOS**: None.

### Sinclair ZX80

- **Emulators**: SZ81 (https://sourceforge.net/projects/sz81/), MAME
- **ROM format**: `.p` (raw RAM dump) or `.rom` (raw 4K/8K ROM image).
- **Install**: `apt install sz81` or `apt install mame`.
- **Debug target**: MAME with `-debug` flag. Launch:
  `mame zx80 -quik game.p -debug`.
- **Required BIOS**: `zx80.rom` (4K or 8K). Place in the emulator ROM
  directory.

### Sinclair ZX81

- **Emulators**: SZ81, MAME
- **ROM format**: `.p81` (17-byte header + RAM dump) or `.rom` (raw 8K
  ROM image).
- **Install**: `apt install sz81` or `apt install mame`.
- **Debug target**: SZ81 has a built-in debugger. MAME:
  `mame zx81 -debug`.
- **Required BIOS**: `zx81.rom` (8K). Place in the emulator ROM
  directory.

### Sinclair Spectrum

- **Emulators**: Fuse (https://fuse-emulator.sourceforge.io/), MAME
- **ROM format**: `.z80` (snapshot with 30/55+ byte header), `.sna`
  (48K snapshot, 27-byte header), `.tap` (tape image).
- **Install**: `apt install fuse` or `apt install mame`.
- **Debug target**: Fuse has a built-in Z80 debugger with breakpoints
  and memory views. MAME: `mame spectrum -debug`.
- **Required BIOS**: `spectrum.rom` (16K 48K ROM). Place in the Fuse
  firmware directory.

### Texas Instruments TI-85

- **Emulators**: TilEm2 (https://github.com/gadgetguru/Tilem)
- **ROM format**: `ti85.rom` (raw 256KB/512KB Z80 ROM dump, no header).
  Programs: `.85z` (ZShell/assembly), `.85p` (TI-85 programs).
- **Install**: `apt install tilem`.
- **Debug target**: TilEm2 has a Z80 debugger with breakpoints, memory
  views, and disassembly. Launch: `tilem85 -rom ti85.rom`.
- **Required BIOS**: `ti85.rom` (the calculator OS ROM). Place in the
  TilEm ROM directory (`~/.tilem/roms/`).