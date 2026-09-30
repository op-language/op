//! Official Nintendo-manual SM83 statement resolver.
//!
//! The code generator consults this resolver only for `sm83` targets
//! and only before the legacy pseudo-mnemonic tables. Classification is
//! pure: it takes a mnemonic and the parser's operand list and matches
//! the statement against the approved opcode matrix. A resolved
//! statement carries its bytes plus the role of the operand that
//! follows them. An official mnemonic whose operand shapes match no
//! matrix row reports a decode diagnostic; the encoder must not fall
//! through to the legacy tables for it, because the legacy path reads
//! only the first operand and would emit wrong bytes for a two-operand
//! statement. Operand-less spellings keep the legacy paths, which
//! emit byte-identical output for every existing spelling.

use op_common::ast::{Expr, Operand};

/// The role of the dynamic operand of a resolved official statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trailing {
    /// One little-endian byte from the value operand
    /// (`ld r, #n`, the `ldh` port byte).
    Value8,
    /// Two little-endian bytes from the value operand
    /// (`ld rr, #nn`, `jp`/`call nn`, `ld (nn), a`, `ld a, (nn)`).
    Value16,
    /// An 8-bit signed relative displacement against a label operand
    /// (`jr cc, 'label`).
    Branch8,
}

/// A statement resolved to an official matrix row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The matched matrix row, for diagnostics and tests.
    pub form: String,
    /// The bytes emitted before the dynamic operand, if any.
    pub bytes: Vec<u8>,
    /// The dynamic operand: its index into the statement's operand
    /// list and the role it plays. `None` when the bytes are complete.
    pub trailing: Option<(usize, Trailing)>,
}

/// The outcome of classifying one SM83 statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The statement is an official form.
    Resolved(Resolved),
    /// The statement begins with an official SM83 mnemonic but its
    /// operand shapes match no matrix row. The message describes the
    /// uncovered shape.
    DecodeError(String),
    /// The statement is not an official form, or is an operand-less
    /// legacy alias. The legacy tables encode it byte for byte.
    NotOfficial,
}

/// The SM83 condition codes with their 2-bit field values: nz=0, z=1,
/// nc=2, c=3. The field sits in bits 3..5 of the conditional-branch
/// opcode: a conditional `jr` uses the base 0x20 (0x20, 0x28, 0x30,
/// 0x38), a conditional `jp` the base 0xC2 (0xC2, 0xCA, 0xD2, 0xDA),
/// and a conditional `ret` the base 0xC0 (0xC0, 0xC8, 0xD0, 0xD8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cond {
    Nz,
    Z,
    Nc,
    C,
}

impl Cond {
    fn code(self) -> u8 {
        match self {
            Self::Nz => 0,
            Self::Z => 1,
            Self::Nc => 2,
            Self::C => 3,
        }
    }

    /// The conditional-branch opcode: `base` with the 2-bit field in
    /// bits 3..5.
    fn jump_op(self, base: u8) -> u8 {
        base | (self.code() << 3)
    }
}

/// The direct-memory forms of the official syntax, classified for the
/// `ld` rows. `(hli)` and `(hld)` are the normalized results of the
/// `(hl+)` and `(hl-)` spellings; `(hl)` is the plain pointer form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Load {
    Hl,
    Hli,
    Hld,
    Bc,
    De,
    /// The I/O port at 0xFF00 + C.
    IoC,
    /// An absolute `(nn)` address or another expression.
    Addr,
}

/// The official SM83 mnemonics. The snake_case pseudo-mnemonics are
/// not in this list, so they keep the legacy tables.
const OFFICIAL_MNEMONICS: &[&str] = &[
    "ld", "ldh", "add", "adc", "sub", "sbc", "and", "or", "xor", "cp", "inc", "dec", "rlc", "rrc",
    "rl", "rr", "sla", "sra", "swap", "srl", "bit", "res", "set", "push", "pop", "jp", "jr",
    "call", "ret", "reti", "rst",
];

/// Return true when the mnemonic is an official SM83 mnemonic.
fn is_official_mnemonic(opcode: &str) -> bool {
    OFFICIAL_MNEMONICS.contains(&opcode)
}

/// Classify one SM83 assembly statement.
pub fn resolve(opcode: &str, operands: &[Operand]) -> Resolution {
    if !is_official_mnemonic(opcode) || operands.is_empty() {
        // Operand-less spellings are legacy aliases (bare `push`,
        // `ret`, `ld_hl`, the snake_case pseudo-mnemonics); the
        // legacy tables encode them byte for byte.
        return Resolution::NotOfficial;
    }
    match opcode {
        "ld" => resolve_ld(operands),
        "ldh" => resolve_ldh(operands),
        "add" | "adc" | "sub" | "sbc" | "and" | "or" | "xor" | "cp" => {
            alu(alu_slot(opcode), opcode, operands)
        }
        "inc" | "dec" => inc_dec(opcode, operands),
        "rlc" | "rrc" | "rl" | "rr" | "sla" | "sra" | "swap" | "srl" => cb_shift(opcode, operands),
        "bit" | "res" | "set" => cb_bit(opcode, operands),
        "push" | "pop" => stack(opcode, operands),
        "jp" => jump_addr(operands),
        "jr" => jump_rel(operands),
        "call" => call(operands),
        "ret" => ret(operands),
        "rst" => rst(operands),
        // The implied entry mnemonics (`nop`, `halt`, `stop`, `di`,
        // `ei`, `daa`, `cpl`, `ccf`, `scf`, `rlca`, `rrca`, `rla`,
        // `rra`, `reti`) take no operands; any operand at all is
        // uncovered.
        _ => decode_op(opcode, "takes no operands"),
    }
}

/// The ALU slot value: add=0, adc=1, sub=2, sbc=3, and=4, xor=5,
/// or=6, cp=7.
fn alu_slot(opcode: &str) -> u8 {
    match opcode {
        "add" => 0,
        "adc" => 1,
        "sub" => 2,
        "sbc" => 3,
        "and" => 4,
        "xor" => 5,
        "or" => 6,
        _ => 7,
    }
}

/// The CB rotation/shift slot: rlc=0, rrc=1, rl=2, rr=3, sla=4,
/// sra=5, swap=6, srl=7.
fn cb_slot(opcode: &str) -> u8 {
    match opcode {
        "rlc" => 0,
        "rrc" => 1,
        "rl" => 2,
        "rr" => 3,
        "sla" => 4,
        "sra" => 5,
        "swap" => 6,
        _ => 7,
    }
}

/// The value of a small constant expression: a number literal, a
/// unary `+`/`-` of a literal, or a parenthesized literal. Symbol and
/// selector folding only exists in the encoder's const tables; an
/// operand that needs it falls to the encoder-side helpers.
fn literal_value(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Number { value } => Some(*value),
        Expr::ParenExpr { inner } => literal_value(inner),
        Expr::UnaryOp { op, operand } => {
            let value = literal_value(operand)?;
            Some(match op {
                op_common::ast::UnaryOp::Neg => -value,
                op_common::ast::UnaryOp::Pos => value,
                op_common::ast::UnaryOp::Not => {
                    if value != 0 {
                        0
                    } else {
                        1
                    }
                }
                op_common::ast::UnaryOp::Inv => !value,
            })
        }
        _ => None,
    }
}

/// Decode one operand that names an 8-bit register or one of the
/// `(hl)` spellings. Register codes: b=0, c=1, d=2, e=3, h=4, l=5,
/// (hl)=6, a=7. Bare register atoms are the identifiers
/// `b c d e h l a`; the `(hl)` slot accepts the `(hli)` / `(hl+)` and
/// `(hld)` / `(hl-)` spellings when `accept_hl_spellings` is true,
/// all of which encode code 6, because the ISA has no
/// auto-incrementing or auto-decrementing variants of these
/// encodings. The general `ld` rows never pass it, so the dedicated
/// `ld (hli), a` and `ld (hld), a` rows stay unambiguous.
fn register_code(operand: &Operand, accept_hl_spellings: bool) -> Option<u8> {
    let Operand::MemoryOperand {
        expr,
        index_reg: None,
        is_indirect,
        ..
    } = operand
    else {
        return None;
    };
    let Expr::Ident { name } = expr else {
        return None;
    };
    match (name.as_str(), *is_indirect) {
        ("b", false) => Some(0),
        ("c", false) => Some(1),
        ("d", false) => Some(2),
        ("e", false) => Some(3),
        ("h", false) => Some(4),
        ("l", false) => Some(5),
        ("a", false) => Some(7),
        // The (hl) spellings are direct-memory forms; the other
        // parenthesized register letters (for example `(b)`) name no
        // official form.
        ("hl", true) => Some(6),
        ("hli" | "hl+", true) if accept_hl_spellings => Some(6),
        ("hld" | "hl-", true) if accept_hl_spellings => Some(6),
        _ => None,
    }
}

/// Return true when the operand is the bare identifier register atom
/// `name` (not a parenthesized form).
fn is_ident_operand(operand: &Operand, name: &str, indirect: bool) -> bool {
    let Operand::MemoryOperand {
        expr: Expr::Ident { name: ident },
        index_reg: None,
        is_indirect,
        ..
    } = operand
    else {
        return false;
    };
    ident == name && *is_indirect == indirect
}

/// Decode one operand that names a 16-bit register pair. Pair codes:
/// bc=0, de=1, hl=2, sp=3.
fn pair_code(operand: &Operand) -> Option<u8> {
    if let Operand::MemoryOperand {
        expr: Expr::Ident { name },
        index_reg: None,
        is_indirect: false,
        ..
    } = operand
    {
        match name.as_str() {
            "bc" => Some(0),
            "de" => Some(1),
            "hl" => Some(2),
            "sp" => Some(3),
            _ => None,
        }
    } else {
        None
    }
}

/// Decode one operand that names a register pair for the stack rows.
/// The pairs stack rows accept: bc=0, de=1, hl=2, af=3. The stack
/// pointer cannot be pushed or popped on the SM83.
fn stack_pair_code(operand: &Operand) -> Option<u8> {
    let Operand::MemoryOperand {
        expr: Expr::Ident { name },
        index_reg: None,
        is_indirect: false,
        ..
    } = operand
    else {
        return None;
    };
    match name.as_str() {
        "af" => Some(3),
        "bc" => Some(0),
        "de" => Some(1),
        "hl" => Some(2),
        _ => None,
    }
}

/// Classify a direct-memory operand for the `ld` rows, distinguishing
/// the `(hli)` / `(hld)` load directions, the register-file pointer
/// forms, the I/O port, and an absolute address.
fn load_direct(operand: &Operand) -> Option<Load> {
    let Operand::MemoryOperand {
        expr,
        index_reg: None,
        is_indirect: true,
        ..
    } = operand
    else {
        return None;
    };
    if let Expr::Ident { name } = expr {
        match name.as_str() {
            "hli" | "hl+" => return Some(Load::Hli),
            "hld" | "hl-" => return Some(Load::Hld),
            "hl" => return Some(Load::Hl),
            "bc" => return Some(Load::Bc),
            "de" => return Some(Load::De),
            "c" => return Some(Load::IoC),
            _ => {}
        }
    }
    Some(Load::Addr)
}

/// Decode the condition-code operand words `nz`, `z`, `nc`, and `c`.
fn condition(operand: &Operand) -> Option<Cond> {
    if let Operand::MemoryOperand {
        expr: Expr::Ident { name },
        index_reg: None,
        is_indirect: false,
        ..
    } = operand
    {
        match name.as_str() {
            "nz" => Some(Cond::Nz),
            "z" => Some(Cond::Z),
            "nc" => Some(Cond::Nc),
            "c" => Some(Cond::C),
            _ => None,
        }
    } else {
        None
    }
}

/// The immediate expression of an operand: the value an `#immediate`
/// carries, or the expression inside a parenthesized address operand.
fn immediate(operand: &Operand) -> Option<&Expr> {
    match operand {
        Operand::Immediate { value } => Some(value),
        Operand::MemoryOperand { expr, .. } => Some(expr),
        _ => None,
    }
}

/// Build a resolved statement with an optional dynamic operand.
fn resolved(
    form: impl Into<String>,
    bytes: Vec<u8>,
    trailing: Option<(usize, Trailing)>,
) -> Resolution {
    Resolution::Resolved(Resolved {
        form: form.into(),
        bytes,
        trailing,
    })
}

/// Build a resolved statement whose bytes are complete.
fn resolved_static(form: &'static str, opcode: u8) -> Resolution {
    resolved(form, vec![opcode], None)
}

/// Build a decode error naming the uncovered SM83 form.
fn decode_op(opcode: &str, detail: &str) -> Resolution {
    Resolution::DecodeError(format!("the official SM83 mnemonic '{opcode}' {detail}"))
}

/// Build a decode error for a form-level uncovered shape.
fn decode_form(form: &str, detail: &str) -> Resolution {
    Resolution::DecodeError(format!(
        "the official SM83 form '{form}' does not cover this operand shape: {detail}"
    ))
}

/// Decode an `ld` statement.
fn resolve_ld(operands: &[Operand]) -> Resolution {
    match operands {
        [dst, src] => {
            // The register-pair rows: ld bc/de/hl/sp, #nn and ld sp, hl.
            if let Some(pair) = pair_code(dst) {
                if matches!(src, Operand::Immediate { .. }) {
                    let (form, opcode) = match pair {
                        0 => ("ld bc, #nn", 0x01),
                        1 => ("ld de, #nn", 0x11),
                        2 => ("ld hl, #nn", 0x21),
                        _ => ("ld sp, #nn", 0x31),
                    };
                    return resolved(form, vec![opcode], Some((1, Trailing::Value16)));
                }
                if pair == 3 && is_ident_operand(src, "hl", false) {
                    return resolved_static("ld sp, hl", 0xF9);
                }
                return decode_form(
                    "ld rr, #nn",
                    "the destination pair loads an immediate, or sp loads hl",
                );
            }
            resolve_ld_registers(dst, src)
        }
        [opd] => {
            // The bare a-implied load `ld #n` is a byte-equal legacy
            // alias of `ld a, #n`.
            if matches!(opd, Operand::Immediate { .. }) {
                return resolved("ld #n", vec![0x3E], Some((0, Trailing::Value8)));
            }
            decode_op(
                "ld",
                "the statement needs a destination and a source operand",
            )
        }
        _ => decode_op("ld", "takes at most two operands"),
    }
}

/// The register and direct-memory `ld` rows. The direct-memory
/// destinations pair only with A (except `(hl)`, which pairs with any
/// 8-bit register or an immediate), and the `(hli)` / `(hld)` source
/// forms have no row, so a wrong-byte legacy fall-through is
/// impossible.
fn resolve_ld_registers(dst: &Operand, src: &Operand) -> Resolution {
    if let Some(dir) = load_direct(dst) {
        match dir {
            // ld (hli), a / ld (hld), a / ld (bc), a / ld (de), a /
            // ld (c), a / ld (nn), a: the store rows pair only with A.
            Load::Hli => return store_a_or_decode("ld (hli), a", 0x22, src),
            Load::Hld => return store_a_or_decode("ld (hld), a", 0x32, src),
            Load::Bc => return store_a_or_decode("ld (bc), a", 0x02, src),
            Load::De => return store_a_or_decode("ld (de), a", 0x12, src),
            Load::IoC => {
                return if register_code(src, false) == Some(7) {
                    resolved_static("ld (c), a", 0xE2)
                } else {
                    decode_form("ld (c), a", "the only encodable source register is a")
                };
            }
            Load::Addr => {
                return if register_code(src, false) == Some(7) {
                    resolved("ld (nn), a", vec![0xEA], Some((0, Trailing::Value16)))
                } else {
                    decode_form("ld (nn), a", "the only encodable source register is a")
                };
            }
            // ld (hl), r and ld (hl), #n. The `(hl), (hl)` shape is
            // excluded: its bytes would collide with the HALT opcode.
            Load::Hl => {
                if let Some(code) = register_code(src, true) {
                    if code == 6 {
                        return decode_form(
                            "ld (hl), r",
                            "(hl), (hl) would collide with the HALT opcode",
                        );
                    }
                    return resolved_static("ld (hl), r", 0x70 | code);
                }
                if matches!(src, Operand::Immediate { .. }) {
                    return resolved("ld (hl), #n", vec![0x36], Some((1, Trailing::Value8)));
                }
                return decode_form(
                    "ld (hl), r",
                    "the source must be a register or an immediate",
                );
            }
        }
    }

    let Some(dst_code) = register_code(dst, false) else {
        return decode_op("ld", "the operand shapes match no matrix form");
    };

    // The (bc)/(de)/(c)/(nn) source forms pair only with A, and the
    // load-direction sources have no row.
    match load_direct(src) {
        Some(Load::Bc) => {
            return if dst_code == 7 {
                resolved_static("ld a, (bc)", 0x0A)
            } else {
                decode_form("ld a, (bc)", "the destination must be a")
            };
        }
        Some(Load::De) => {
            return if dst_code == 7 {
                resolved_static("ld a, (de)", 0x1A)
            } else {
                decode_form("ld a, (de)", "the destination must be a")
            };
        }
        Some(Load::IoC) => {
            return if dst_code == 7 {
                resolved_static("ld a, (c)", 0xF2)
            } else {
                decode_form("ld a, (c)", "the destination must be a")
            };
        }
        Some(Load::Addr) => {
            return if dst_code == 7 {
                resolved("ld a, (nn)", vec![0xFA], Some((1, Trailing::Value16)))
            } else {
                decode_form("ld a, (nn)", "the destination must be a")
            };
        }
        Some(Load::Hli) => {
            return decode_form(
                "ld a, (hl)",
                "the (hli) spelling has no load row; `ld (hli), a` is the store form",
            );
        }
        Some(Load::Hld) => {
            return decode_form(
                "ld a, (hl)",
                "the (hld) spelling has no load row; `ld (hld), a` is the store form",
            );
        }
        // The plain (hl) form is register code 6.
        Some(Load::Hl) | None => {}
    }

    // The immediate-source row: ld r, #n = 0x06 | 8r (ld a, #n =
    // 0x3E). Only a `#immediate` source selects it.
    if matches!(src, Operand::Immediate { .. }) {
        return resolved(
            "ld r, #n",
            vec![0x06 | (8 * dst_code)],
            Some((1, Trailing::Value8)),
        );
    }

    let Some(src_code) = register_code(src, false) else {
        return decode_op("ld", "the operand shapes match no matrix form");
    };
    if dst_code == 6 && src_code == 6 {
        return decode_op("ld", "(hl), (hl) would collide with the HALT opcode");
    }
    resolved_static("ld r, r", 0x40 | (8 * dst_code) | src_code)
}

/// The `ld (x), a` store rows: only the A register is encodable.
fn store_a_or_decode(form: &'static str, opcode: u8, src: &Operand) -> Resolution {
    if register_code(src, false) == Some(7) {
        resolved_static(form, opcode)
    } else {
        decode_form(form, "the only encodable source register is a")
    }
}

/// Decode an `ldh` statement. The official forms are two-operand:
/// `ldh (n), a` (0xE0) and `ldh a, (n)` (0xF0). The one-operand
/// spellings `ldh (n)` and `ldh #n` are byte-equal legacy aliases that
/// stay on the legacy path.
fn resolve_ldh(operands: &[Operand]) -> Resolution {
    match operands {
        [dst, src] => {
            if load_direct(dst).map(|d| matches!(d, Load::Addr)) == Some(true)
                && register_code(src, false) == Some(7)
            {
                return resolved("ldh (n), a", vec![0xE0], Some((0, Trailing::Value8)));
            }
            if register_code(dst, false) == Some(7)
                && load_direct(src).map(|d| matches!(d, Load::Addr)) == Some(true)
            {
                return resolved("ldh a, (n)", vec![0xF0], Some((1, Trailing::Value8)));
            }
            decode_form("ldh (n), a", "the port byte pairs only with the A register")
        }
        [_opd] => {
            // The single-operand spellings are legacy aliases.
            Resolution::NotOfficial
        }
        _ => decode_op("ldh", "takes at most two operands"),
    }
}

/// Decode an ALU statement. The ALU slot s (add=0, adc=1, sub=2,
/// sbc=3, and=4, xor=5, or=6, cp=7) encodes: op a, r or op (hl) = 0x80
/// | 8s | r; op a, #n and the a-implied one-operand forms op r and
/// op #n (= C6 | 8s) share the row.
fn alu(slot: u8, opcode: &str, operands: &[Operand]) -> Resolution {
    match operands {
        [dst, src] => {
            if register_code(dst, false) == Some(7) {
                if let Some(code) = register_code(src, true) {
                    return resolved(
                        format!("{opcode} a, r"),
                        vec![0x80 | (8 * slot) | code],
                        None,
                    );
                }
                if matches!(src, Operand::Immediate { .. }) {
                    return resolved(
                        format!("{opcode} a, #n"),
                        vec![0xC6 | (8 * slot)],
                        Some((1, Trailing::Value8)),
                    );
                }
            }
            decode_op(opcode, "the operand shapes match no matrix form")
        }
        [opd] => {
            if let Some(code) = register_code(opd, true) {
                return resolved(format!("{opcode} r"), vec![0x80 | (8 * slot) | code], None);
            }
            if matches!(opd, Operand::Immediate { .. }) {
                return resolved(
                    format!("{opcode} #n"),
                    vec![0xC6 | (8 * slot)],
                    Some((0, Trailing::Value8)),
                );
            }
            decode_op(opcode, "the operand shapes match no matrix form")
        }
        _ => decode_op(opcode, "takes at most two operands"),
    }
}

/// Decode an `inc` / `dec` statement: register = base + 8r, pair =
/// base + 16 * pair.
fn inc_dec(opcode: &str, operands: &[Operand]) -> Resolution {
    let (base8, base_pair) = if opcode == "inc" {
        (0x04, 0x03)
    } else {
        (0x05, 0x0B)
    };
    match operands {
        [operand] => {
            if let Some(pair) = pair_code(operand) {
                return resolved(format!("{opcode} rr"), vec![base_pair | (pair << 4)], None);
            }
            if let Some(code) = register_code(operand, true) {
                return resolved(format!("{opcode} r"), vec![base8 | (8 * code)], None);
            }
            decode_op(opcode, "the operand shapes match no matrix form")
        }
        _ => decode_op(opcode, "takes a single register or pair operand"),
    }
}

/// Decode a CB rotation or shift: CB followed by (8 * slot) + code.
fn cb_shift(opcode: &str, operands: &[Operand]) -> Resolution {
    let slot = cb_slot(opcode);
    match operands {
        [operand] => {
            if let Some(code) = register_code(operand, true) {
                return resolved(format!("{opcode} r"), vec![0xCB, (8 * slot) | code], None);
            }
            decode_op(opcode, "the operand shapes match no matrix form")
        }
        _ => decode_op(opcode, "takes a single register operand"),
    }
}

/// Decode a CB bit test/set/reset: CB and then (prefix | 8n) + code,
/// with prefix 0x40 for `bit`, 0x80 for `res`, and 0xC0 for `set`.
/// The bit number must be a constant in 0..=7.
fn cb_bit(opcode: &str, operands: &[Operand]) -> Resolution {
    let prefix = match opcode {
        "bit" => 0x40,
        "res" => 0x80,
        _ => 0xC0,
    };
    match operands {
        [n, register] => {
            let Some(bit) = immediate(n).and_then(literal_value) else {
                return decode_op(opcode, "the bit number must be a constant 0..=7");
            };
            if !(0..=7).contains(&bit) {
                return decode_op(opcode, "the bit number must be 0..=7");
            }
            let Some(code) = register_code(register, true) else {
                return decode_op(opcode, "the operand shapes match no matrix form");
            };
            resolved(
                format!("{opcode} n, r"),
                vec![0xCB, prefix | (8 * bit as u8) | code],
                None,
            )
        }
        _ => decode_op(opcode, "takes a bit number and a register operand"),
    }
}

/// Decode a `push` / `pop` statement.
fn stack(opcode: &str, operands: &[Operand]) -> Resolution {
    let base = if opcode == "push" { 0xC5 } else { 0xC1 };
    match operands {
        [operand] => match stack_pair_code(operand) {
            Some(pair) => resolved(format!("{opcode} rr"), vec![base | (pair << 4)], None),
            None => decode_op(opcode, "the operand shapes match no matrix form"),
        },
        _ => decode_op(opcode, "takes a single register-pair operand"),
    }
}

/// Decode an absolute jump: `jp nn` (0xC3), `jp cc, nn` (0xC2 base),
/// `call nn` (0xCD, with no condition form). A `(hl)` destination is
/// an out-of-scope form and is a decode error rather than a fall
/// through to a wrong legacy entry.
fn jump_addr(operands: &[Operand]) -> Resolution {
    match operands {
        [target] => address_family(target, None, "jp nn", 0xC3, 0),
        [cond, target] => match condition(cond) {
            Some(code) => address_family(target, Some(code.jump_op(0xC2)), "jp cc, nn", 0xC3, 1),
            None => decode_op(
                "jp",
                "the first operand of the conditional form is a condition",
            ),
        },
        _ => decode_op("jp", "takes a label or an address operand"),
    }
}

fn jump_rel(operands: &[Operand]) -> Resolution {
    match operands {
        [target] => {
            if let Operand::LabelRef { .. } = target {
                resolved("jr", vec![0x18], Some((0, Trailing::Branch8)))
            } else {
                decode_op("jr", "takes a label target")
            }
        }
        [cond, target] => match condition(cond) {
            Some(code) => match target {
                Operand::LabelRef { .. } => resolved(
                    "jr cc, label",
                    vec![code.jump_op(0x20)],
                    Some((1, Trailing::Branch8)),
                ),
                _ => decode_op("jr", "the operand shapes match no matrix form"),
            },
            None => decode_op(
                "jr",
                "the first operand of the conditional form is a condition",
            ),
        },
        _ => decode_op("jr", "takes a label target"),
    }
}

/// Decode `call nn`: a label or an address operand. The conditional
/// call form is not covered by the matrix.
fn call(operands: &[Operand]) -> Resolution {
    match operands {
        [target] => address_family(target, None, "call nn", 0xCD, 0),
        [_cond, _target] => decode_op(
            "call",
            "the conditional call form is not covered by the matrix",
        ),
        _ => decode_op("call", "takes a label or an address operand"),
    }
}

/// The `jp` / `call` forms: a label target reuses the Abs16
/// relocation plumbing, and an expression target folds to a constant
/// or a relocation. `conditional_op` carries the conditional-form
/// opcode (`None` for the unconditional row); `value_index` is the
/// operand the address value belongs to.
fn address_family(
    target: &Operand,
    conditional_op: Option<u8>,
    form: &'static str,
    unconditional: u8,
    value_index: usize,
) -> Resolution {
    match target {
        Operand::LabelRef { .. } => {}
        Operand::MemoryOperand {
            expr: Expr::Ident { name },
            index_reg: None,
            is_indirect: true,
            ..
        } if matches!(name.as_str(), "hl" | "hli" | "hld" | "bc" | "de" | "c") => {
            // Direct-memory destinations such as `jp (hl)` have no
            // covered form.
            return decode_form(
                form,
                "the direct-memory destination is an out-of-scope form",
            );
        }
        operand => {
            if immediate(operand).is_none() {
                return decode_form(form, "the target must be a label or an address");
            }
        }
    }
    let opcode = conditional_op.unwrap_or(unconditional);
    resolved(form, vec![opcode], Some((value_index, Trailing::Value16)))
}

/// Decode `ret cc`: 0xC0 with the condition's 2-bit field in bits
/// 3..5. The operand-less `ret` is a legacy alias.
fn ret(operands: &[Operand]) -> Resolution {
    match operands {
        [cond] => match condition(cond) {
            Some(code) => resolved("ret cc", vec![code.jump_op(0xC0)], None),
            None => decode_op("ret", "the only one-operand form takes a condition code"),
        },
        _ => decode_op("ret", "takes a single condition-code operand"),
    }
}

/// Decode `rst n`: 0xC7 with the encoded 3-bit restart vector. The
/// vector must be a constant in 0x00..=0x38 that is a multiple of 8;
/// the encoded byte is 0xC7 | (n & 0x38).
fn rst(operands: &[Operand]) -> Resolution {
    match operands {
        [operand] => {
            let Some(vector) = immediate(operand).and_then(literal_value) else {
                return decode_op("rst", "the restart vector must be a constant");
            };
            if !(0x00..=0x38).contains(&vector) || vector % 8 != 0 {
                return decode_op(
                    "rst",
                    "the restart vector must be one of 0x00, 0x08, ... 0x38",
                );
            }
            resolved("rst n", vec![0xC7 | (vector as u8 & 0x38)], None)
        }
        _ => decode_op("rst", "takes a single constant vector operand"),
    }
}

// --- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A bare register or register-pair atom.
    fn reg(name: &str) -> Operand {
        Operand::MemoryOperand {
            mode_prefix: None,
            expr: Expr::Ident { name: name.into() },
            index_reg: None,
            is_indirect: false,
        }
    }

    /// A parenthesized direct-memory operand.
    fn mem(expr: Expr) -> Operand {
        Operand::MemoryOperand {
            mode_prefix: None,
            expr,
            index_reg: None,
            is_indirect: true,
        }
    }

    /// A parenthesized register-named direct-memory operand.
    fn mem_reg(name: &str) -> Operand {
        mem(Expr::Ident { name: name.into() })
    }

    /// An `#immediate` operand.
    fn imm_value(value: i64) -> Operand {
        Operand::Immediate {
            value: Expr::Number { value },
        }
    }

    /// A parenthesized constant-address operand.
    fn addr(value: i64) -> Operand {
        mem(Expr::Number { value })
    }

    /// A label-target operand.
    fn label(name: &str) -> Operand {
        Operand::LabelRef { name: name.into() }
    }

    /// The bytes of a resolved statement, or `None` when the
    /// statement is not an official form.
    fn bytes(opcode: &str, operands: &[Operand]) -> Option<Vec<u8>> {
        match resolve(opcode, operands) {
            Resolution::Resolved(resolved) => Some(resolved.bytes),
            _ => None,
        }
    }

    /// Assert that the statement decodes to an error.
    fn assert_error(opcode: &str, operands: &[Operand]) {
        assert!(
            matches!(resolve(opcode, operands), Resolution::DecodeError(_)),
            "expected a decode error for '{opcode}'"
        );
    }

    // The ld matrix.
    #[test]
    fn ld_matrix() {
        assert_eq!(
            bytes("ld", &[reg("sp"), imm_value(0xFFFE)]),
            Some(vec![0x31])
        );
        assert_eq!(
            bytes("ld", &[reg("hl"), imm_value(0x9FFF)]),
            Some(vec![0x21])
        );
        assert_eq!(
            bytes("ld", &[reg("bc"), imm_value(0x1234)]),
            Some(vec![0x01])
        );
        assert_eq!(
            bytes("ld", &[reg("de"), imm_value(0x1234)]),
            Some(vec![0x11])
        );
        assert_eq!(bytes("ld", &[reg("a"), imm_value(0x34)]), Some(vec![0x3E]));
        assert_eq!(bytes("ld", &[reg("b"), imm_value(0x34)]), Some(vec![0x06]));
        assert_eq!(bytes("ld", &[reg("h"), imm_value(0)]), Some(vec![0x26]));
        assert_eq!(bytes("ld", &[mem_reg("hli"), reg("a")]), Some(vec![0x22]));
        assert_eq!(bytes("ld", &[mem_reg("hl+"), reg("a")]), Some(vec![0x22]));
        assert_eq!(bytes("ld", &[mem_reg("hld"), reg("a")]), Some(vec![0x32]));
        assert_eq!(bytes("ld", &[mem_reg("bc"), reg("a")]), Some(vec![0x02]));
        assert_eq!(bytes("ld", &[mem_reg("de"), reg("a")]), Some(vec![0x12]));
        assert_eq!(bytes("ld", &[mem_reg("c"), reg("a")]), Some(vec![0xE2]));
        assert_eq!(bytes("ld", &[reg("a"), reg("b")]), Some(vec![0x78]));
        assert_eq!(bytes("ld", &[reg("c"), reg("a")]), Some(vec![0x4F]));
        assert_eq!(bytes("ld", &[reg("l"), reg("c")]), Some(vec![0x69]));
        assert_eq!(bytes("ld", &[mem_reg("hl"), reg("b")]), Some(vec![0x70]));
        assert_eq!(bytes("ld", &[mem_reg("hl"), reg("a")]), Some(vec![0x77]));
        assert_eq!(bytes("ld", &[reg("b"), mem_reg("hl")]), Some(vec![0x46]));
        assert_eq!(bytes("ld", &[reg("a"), mem_reg("hl")]), Some(vec![0x7E]));
        assert_eq!(
            bytes("ld", &[mem_reg("hl"), imm_value(0x34)]),
            Some(vec![0x36])
        );
        assert_eq!(bytes("ld", &[addr(0x2134), reg("a")]), Some(vec![0xEA]));
        assert_eq!(bytes("ld", &[reg("a"), addr(0x2134)]), Some(vec![0xFA]));
        assert_eq!(bytes("ld", &[reg("a"), mem_reg("bc")]), Some(vec![0x0A]));
        assert_eq!(bytes("ld", &[reg("a"), mem_reg("de")]), Some(vec![0x1A]));
        assert_eq!(bytes("ld", &[reg("a"), mem_reg("c")]), Some(vec![0xF2]));
        assert_eq!(bytes("ld", &[reg("sp"), reg("hl")]), Some(vec![0xF9]));
    }

    // The ALU matrix.
    #[test]
    fn alu_matrix() {
        assert_eq!(bytes("xor", &[reg("a")]), Some(vec![0xAF]));
        assert_eq!(bytes("xor", &[reg("a"), reg("a")]), Some(vec![0xAF]));
        assert_eq!(bytes("add", &[reg("a"), mem_reg("hl")]), Some(vec![0x86]));
        assert_eq!(bytes("sub", &[reg("b")]), Some(vec![0x90]));
        assert_eq!(bytes("cp", &[mem_reg("hl")]), Some(vec![0xBE]));
        assert_eq!(bytes("cp", &[imm_value(144)]), Some(vec![0xFE]));
        assert_eq!(bytes("and", &[reg("a"), imm_value(2)]), Some(vec![0xE6]));
        assert_eq!(bytes("or", &[reg("c")]), Some(vec![0xB1]));
        assert_eq!(bytes("adc", &[reg("a"), imm_value(8)]), Some(vec![0xCE]));
        assert_eq!(bytes("sbc", &[reg("a"), reg("e")]), Some(vec![0x9B]));
    }

    // The INC/DEC rows.
    #[test]
    fn inc_dec_matrix() {
        assert_eq!(bytes("inc", &[reg("hl")]), Some(vec![0x23]));
        assert_eq!(bytes("inc", &[reg("de")]), Some(vec![0x13]));
        assert_eq!(bytes("inc", &[mem_reg("hl")]), Some(vec![0x34]));
        assert_eq!(bytes("inc", &[reg("b")]), Some(vec![0x04]));
        assert_eq!(bytes("dec", &[reg("h")]), Some(vec![0x25]));
        assert_eq!(bytes("dec", &[reg("sp")]), Some(vec![0x3B]));
    }

    // The CB rows.
    #[test]
    fn cb_matrix() {
        assert_eq!(bytes("rl", &[reg("c")]), Some(vec![0xCB, 0x11]));
        assert_eq!(bytes("rlc", &[reg("a")]), Some(vec![0xCB, 0x07]));
        assert_eq!(bytes("sra", &[reg("b")]), Some(vec![0xCB, 0x28]));
        assert_eq!(bytes("swap", &[mem_reg("hli")]), Some(vec![0xCB, 0x36]));
        assert_eq!(bytes("srl", &[reg("a")]), Some(vec![0xCB, 0x3F]));
        assert_eq!(
            bytes("bit", &[imm_value(7), reg("h")]),
            Some(vec![0xCB, 0x7C])
        );
        assert_eq!(
            bytes("bit", &[imm_value(0), reg("c")]),
            Some(vec![0xCB, 0x41])
        );
        assert_eq!(
            bytes("res", &[imm_value(3), mem_reg("hli")]),
            Some(vec![0xCB, 0x9E])
        );
        assert_eq!(
            bytes("set", &[imm_value(3), reg("c")]),
            Some(vec![0xCB, 0xD9])
        );
    }

    // The stack and control rows.
    #[test]
    fn stack_and_control_matrix() {
        assert_eq!(bytes("push", &[reg("af")]), Some(vec![0xF5]));
        assert_eq!(bytes("push", &[reg("bc")]), Some(vec![0xC5]));
        assert_eq!(bytes("pop", &[reg("hl")]), Some(vec![0xE1]));
        assert_eq!(bytes("pop", &[reg("de")]), Some(vec![0xD1]));
        assert_eq!(bytes("jp", &[addr(0x0050)]), Some(vec![0xC3]));
        assert_eq!(bytes("jp", &[label("Boot")]), Some(vec![0xC3]));
        assert_eq!(bytes("jp", &[reg("nz"), label("Boot")]), Some(vec![0xC2]));
        assert_eq!(bytes("jp", &[reg("c"), label("Boot")]), Some(vec![0xDA]));
        assert_eq!(bytes("jr", &[label("Loop")]), Some(vec![0x18]));
        assert_eq!(bytes("jr", &[reg("nz"), label("Loop")]), Some(vec![0x20]));
        assert_eq!(bytes("jr", &[reg("z"), label("Loop")]), Some(vec![0x28]));
        assert_eq!(bytes("jr", &[reg("nc"), label("Loop")]), Some(vec![0x30]));
        assert_eq!(bytes("jr", &[reg("c"), label("Loop")]), Some(vec![0x38]));
        assert_eq!(bytes("call", &[label("Fn")]), Some(vec![0xCD]));
        assert_eq!(bytes("ret", &[reg("nz")]), Some(vec![0xC0]));
        assert_eq!(bytes("ret", &[reg("z")]), Some(vec![0xC8]));
        assert_eq!(bytes("ret", &[reg("nc")]), Some(vec![0xD0]));
        assert_eq!(bytes("ret", &[reg("c")]), Some(vec![0xD8]));
        assert_eq!(bytes("rst", &[imm_value(0x00)]), Some(vec![0xC7]));
        assert_eq!(bytes("rst", &[imm_value(0x38)]), Some(vec![0xFF]));
    }

    // The one-operand byte-equal legacy aliases keep the legacy path
    // (or resolve to the same byte the legacy table emits).
    #[test]
    fn legacy_aliases_stay_legacy() {
        assert_eq!(bytes("ld", &[imm_value(0x34)]), Some(vec![0x3E]));
        assert!(matches!(
            resolve("ldh", &[imm_value(0x44)]),
            Resolution::NotOfficial
        ));
        assert!(matches!(resolve("push", &[]), Resolution::NotOfficial));
        assert!(matches!(resolve("ret", &[]), Resolution::NotOfficial));
        assert!(matches!(resolve("ldi", &[]), Resolution::NotOfficial));
        assert!(matches!(resolve("nop", &[]), Resolution::NotOfficial));
        assert!(matches!(
            resolve("ld_hl", &[imm_value(5)]),
            Resolution::NotOfficial
        ));
    }

    // Uncovered official shapes decode to an error instead of falling
    // through to a legacy entry that would emit wrong bytes.
    #[test]
    fn uncovered_shapes_are_decode_errors() {
        // The legacy path would silently emit 0x32 for these shapes.
        assert_error("ld", &[mem_reg("hli"), reg("b")]);
        assert_error("ld", &[reg("a"), mem_reg("hli")]);
        assert_error("ld", &[mem_reg("hld"), reg("b")]);
        assert_error("ld", &[addr(0x2134), reg("b")]);
        // (hl), (hl) collides with the HALT opcode.
        assert_error("ld", &[mem_reg("hl"), mem_reg("hl")]);
        assert_error("ld", &[reg("af"), imm_value(0)]);
        assert_error("bit", &[imm_value(8), reg("h")]);
        assert_error("push", &[reg("sp")]);
        assert_error("rst", &[imm_value(0x39)]);
        assert_error("jr", &[imm_value(0x34)]);
        assert_error("jp", &[mem_reg("hl")]);
        assert_error("call", &[reg("nz"), label("Fn")]);
        assert_error("inc", &[reg("af")]);
        // Parenthesized 8-bit register letters name no official form.
        assert_error("ld", &[mem_reg("hl"), mem_reg("b")]);
        assert_error("inc", &[mem_reg("b")]);
        assert_error("xor", &[mem_reg("a")]);
    }

    // The operand roles: each resolved statement names the operand
    // that carries its dynamic value.
    #[test]
    fn trailing_roles() {
        let Resolution::Resolved(resolved) = resolve("ld", &[reg("bc"), imm_value(0x1234)]) else {
            panic!("expected the ld bc, #nn statement to resolve");
        };
        assert_eq!(resolved.form, "ld bc, #nn");
        assert_eq!(resolved.trailing, Some((1, Trailing::Value16)));

        let Resolution::Resolved(resolved) = resolve("ldh", &[addr(0x47), reg("a")]) else {
            panic!("expected the ldh (n), a statement to resolve");
        };
        assert_eq!(resolved.form, "ldh (n), a");
        assert_eq!(resolved.trailing, Some((0, Trailing::Value8)));

        let Resolution::Resolved(resolved) = resolve("jr", &[label("Loop")]) else {
            panic!("expected the jr statement to resolve");
        };
        assert_eq!(resolved.form, "jr");
        assert_eq!(resolved.trailing, Some((0, Trailing::Branch8)));

        let Resolution::Resolved(resolved) = resolve("ld", &[mem_reg("hl"), reg("a")]) else {
            panic!("expected the ld (hl), a statement to resolve");
        };
        assert_eq!(resolved.form, "ld (hl), r");
        assert_eq!(resolved.trailing, None);
    }
}
