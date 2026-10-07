// Op grammar for tree-sitter
//
// Models the normalized LR(1) grammar in
// `op/docs/language-specification.md` (lines 1999-2190) and matches the
// compiler in `op/crates/opc/`.
//
// Op is a high-level assembler for retro game consoles. Source files use
// the `.op` extension. A statement-leading word parses as an opcode when it
// matches one of the CPU-family mnemonic sets in the lexer's OPCODES list;
// the grammar models the lowercase forms. Any other statement-leading word
// takes the variable-declaration reading, like the compiler's statement
// dispatch.

const OPCODE_6502 = [
  'adc', 'and', 'asl', 'bcc', 'bcs', 'beq', 'bit', 'bmi', 'bne', 'bpl', 'brk',
  'bvc', 'bvs', 'clc', 'cld', 'cli', 'clv', 'cmp', 'cpx', 'cpy', 'dec', 'dex',
  'dey', 'eor', 'inc', 'inx', 'iny', 'jmp', 'jsr', 'lda', 'ldx', 'ldy', 'lsr',
  'nop', 'ora', 'pha', 'php', 'pla', 'plp', 'rol', 'ror', 'rti', 'rts', 'sbc',
  'sec', 'sed', 'sei', 'sta', 'stx', 'sty', 'tax', 'tay', 'tsx', 'txa', 'txs',
  'tya',
  // undocumented
  'alr', 'anc', 'ane', 'arr', 'dcp', 'isc', 'las', 'lax', 'lxa', 'rla', 'rra',
  'sax', 'sha', 'shx', 'shy', 'slo', 'sre', 'tas', 'usbc',
];

const OPCODE_65SC02 = [
  'bra', 'phx', 'phy', 'plx', 'ply', 'stz', 'tsb', 'trb', 'ina', 'dea',
];

const OPCODE_W65C02 = [
  // W65C02 Rockwell bit operations (wai and stp are shared with the 65C816)
  'rmb0', 'rmb1', 'rmb2', 'rmb3', 'rmb4', 'rmb5', 'rmb6', 'rmb7',
  'smb0', 'smb1', 'smb2', 'smb3', 'smb4', 'smb5', 'smb6', 'smb7',
  'bbr0', 'bbr1', 'bbr2', 'bbr3', 'bbr4', 'bbr5', 'bbr6', 'bbr7',
  'bbs0', 'bbs1', 'bbs2', 'bbs3', 'bbs4', 'bbs5', 'bbs6', 'bbs7',
];

const OPCODE_65C816 = [
  'rep', 'sep', 'xba', 'xce', 'tcd', 'tdc', 'tcs', 'tsc', 'txy', 'tyx', 'mvn',
  'mvp', 'pea', 'pei', 'per', 'jml', 'jsl', 'rtl', 'cop', 'wai', 'stp',
  // Declared by the W65C816 std CPU library only, not yet lexer opcode
  // tokens; kept in the grammar so library sources parse.
  'phb', 'phd', 'phk', 'plb', 'pld', 'tad', 'tda', 'tsa', 'wdm',
];

const OPCODE_68000 = [
  'move', 'moveq', 'movem', 'lea', 'clr', 'not', 'or', 'eor', 'add', 'adda',
  'addi', 'addq', 'sub', 'suba', 'subi', 'subq', 'mulu', 'muls', 'divu',
  'divs', 'neg', 'negx', 'abs', 'asr', 'lsl', 'lsr', 'ror', 'roxl', 'roxr',
  'cmpa', 'cmpi', 'tst', 'btst', 'bset', 'bclr', 'bchg', 'rtr', 'rte', 'bcc',
  'bsr', 'dbcc', 'chk', 'trap', 'trapv', 'swap', 'exg', 'ext', 'link', 'unlk',
  'reset', 'stop', 'illegal',
];

const OPCODE_Z80 = [
  'ld', 'push', 'pop', 'ex', 'exx', 'ldi', 'ldir', 'ldd', 'lddr', 'cpi', 'cpir',
  'cpd', 'cpdr', 'adc', 'sbc', 'cp', 'inc', 'dec', 'daa', 'cpl', 'neg', 'ccf',
  'scf', 'halt', 'di', 'ei', 'im', 'rlc', 'rl', 'rrc', 'rr', 'sla', 'sra',
  'sll', 'srl', 'rld', 'rrd', 'rlca', 'rrca', 'rra', 'jp', 'jr', 'djnz',
  'call', 'ret', 'reti', 'retn', 'rst', 'in', 'out', 'ini', 'inir', 'ind',
  'indr', 'outi', 'otir', 'outd', 'otdr', 'bit', 'set', 'res', 'xor',
];

const OPCODE_LR35902 = [
  'stop', 'ldi', 'ldd', 'ldh',
  // SM83 register-pair underscore pseudo-instructions
  'inc_hl', 'inc_de', 'inc_bc', 'ld_hl', 'ld_de', 'ld_bc', 'ld_a_hl',
  'ld_a_bc', 'ld_a_de', 'ld_ba', 'ld_ca', 'ld_da', 'ld_ea', 'ld_ha', 'ld_la',
  'ld_ab', 'ld_ac', 'ld_ad', 'ld_ae', 'ld_ah', 'ld_al', 'ld_a', 'ld_addr',
  'ld_sp', 'ld_hl_a', 'ld_hld_a', 'ld_c_a', 'ld_b', 'ld_c', 'ld_d', 'ld_e',
  'ld_h', 'ld_l', 'inc_b', 'inc_c', 'inc_d', 'inc_e', 'inc_h', 'inc_l',
  'inc_a', 'dec_b', 'dec_c', 'dec_d', 'dec_e', 'dec_h', 'dec_l', 'dec_a',
  'add_a_hl', 'sub_b', 'xor_a', 'bit_h7', 'rl_c', 'cp_hl', 'jr_nz', 'jr_z',
  'jr_nc', 'jr_c',
];

const OPCODES = [
  ...OPCODE_6502,
  ...OPCODE_65SC02,
  ...OPCODE_W65C02,
  ...OPCODE_65C816,
  ...OPCODE_68000,
  ...OPCODE_Z80,
  ...OPCODE_LR35902,
];

const CONDITION_KEYWORDS = [
  // 6502 family
  'plus', 'positive', 'minus', 'negative', 'greater', 'less', 'overflow',
  'carry', 'nonzero', 'set', 'zero', 'unset', 'clear', 'equal',
  // 68000
  'high', 'low_or_same', 'carry_clear', 'carry_set', 'not_equal',
  'overflow_clear', 'overflow_set', 'greater_or_equal', 'less_than',
  'greater_than', 'less_or_equal',
  // z80
  'not_zero', 'no_carry', 'parity_even', 'parity_odd', 'sign_positive',
  'sign_negative',
  // true and false are keywords in the compiler and never condition tokens
];

const CONDITION_MODIFIERS = ['is', 'has', 'no', 'not'];

const KEYWORDS = [
  'fn', 'inline', 'noreturn', 'return', 'volatile', 'struct', 'type', 'enum',
  'const', 'mod', 'use', 'pub', 'lib', 'self', 'super', 'if', 'else', 'while',
  'do', 'loop', 'switch', 'case', 'default', 'near', 'far', 'as',
];

const PRIMITIVE_TYPES = [
  'u8', 'i8', 'u16', 'i16', 'u32', 'i32', 'bool', 'pointer',
];

const MODE_PREFIXES = ['zp', 'abs', 'rel', 'ind', 'idx', 'ind_l', 'ind_idx'];

// Reference data for the attribute vocabulary. The `attr_path` rule stays
// identifier-driven; this const is kept for documentation and tooling.
const ATTR_NAMES = [
  'cfg', 'crash_handler', 'interrupt', 'addr', 'rom', 'ram', 'chr', 'align',
  'setpad', 'ines', 'lnx', 'loader', 'gb', 'snes', 'sega', 'sms', 'a78',
  'crt', 'locate',
];

const COMPILE_MACROS = [
  'lo', 'hi', 'nylo', 'nyhi', 'sizeof', 'len', 'compile_error', 'assert',
  'assert_eq', 'debug_assert', 'debug_assert_eq', 'panic',
];

const INCLUDE_MACROS = ['locate_str', 'font_load'];

module.exports = grammar({
  name: 'op',

  word: $ => $.identifier,

  extras: $ => [
    /\s/,
    $.module_doc_comment,
    $.doc_comment,
    $.line_comment,
    $.block_comment,
  ],

  conflicts: $ => [
    [$.assembly_stmt, $.assembly_stmt],
    // A `, index_reg` continuation and a new operand both start at the same
    // comma after an indexed operand; the compiler binds only x, y, cpu::x,
    // and cpu::y, so both readings stay alive and the dynamic precedence
    // below picks the bound reading.
    [$.memory_operand, $.memory_operand],
    [$.memory_operand, $._primary],
    [$.init_list, $._primary],
    [$._operand, $._primary],
    [$._selector_start, $.path],
  ],

  rules: {
    // --- Source unit --------------------------------------------------------

    source_file: $ => repeat($._module_item),

    _module_item: $ => choice(
      $.module_doc_comment,
      $.attribute,
      $._item,
    ),

    _item: $ => choice(
      $.const_decl,
      $.var_decl,
      $.fn_decl,
      $.inline_fn_decl,
      $.struct_decl,
      $.type_decl,
      $.enum_decl,
      $.mod_decl,
      $.use_decl,
      $.block_attribute,
      $.placement,
    ),

    // --- Attributes ---------------------------------------------------------

    attribute: $ => seq(
      '#[',
      $.attr_path,
      optional($.attr_args),
      ']',
    ),

    attr_path: $ => sep1($._attr_path_segment, '::'),
    _attr_path_segment: $ => $.identifier,

    attr_args: $ => seq('(', sep1($.attr_arg, ','), optional(','), ')'),

    attr_arg: $ => choice(
      $.identifier,
      $.literal,
      $.attr_dotted_key,
      seq($.identifier, '=', $.literal),
      $.attr_combinator,
    ),

    // Dotted attribute keys such as ines.mapper; a dotted name may carry the
    // key=value form the cfg predicates use for header fields (for example
    // ines.mapper = "nrom").
    attr_dotted_key: $ => seq(
      $.identifier,
      repeat1(seq('.', $.identifier)),
      optional(seq('=', $.literal)),
    ),

    // Combinator arguments with nested argument lists; `not` reaches the
    // parser as a modifier token, so it needs a literal form beside bare
    // identifiers (the compiler accepts the shape for any name).
    attr_combinator: $ => seq(
      choice($.identifier, 'not'),
      '(',
      sep1($.attr_arg, ','),
      optional(','),
      ')',
    ),

    block_attribute: $ => seq(
      $.attribute,
      '{',
      repeat($._module_item),
      '}',
    ),

    placement: $ => seq(
      $.include_macro_call,
      optional(';'),
    ),

    // --- Declarations -------------------------------------------------------

    const_decl: $ => seq(
      optional('pub'),
      'const',
      field('name', $.identifier),
      ':',
      field('type', $._type),
      '=',
      field('value', $._expr),
      optional(';'),
    ),

    var_decl: $ => seq(
      optional('pub'),
      optional('volatile'),
      field('name', $.identifier),
      ':',
      field('type', $._type),
      optional($.array_dim),
      optional($.addr_binding),
      optional($.init_value),
      optional(';'),
    ),

    addr_binding: $ => seq(':', $._expr),

    init_value: $ => prec.left(seq(
      '=',
      choice(
        $._expr,
        $.init_list,
      ),
    )),

    init_list: $ => prec.left(seq(
      '{',
      optional(seq(
        sep1(choice($._expr, $.init_list, $.string), ','),
        optional(','),
      )),
      '}',
    )),

    fn_decl: $ => seq(
      optional('pub'),
      optional('noreturn'),
      'fn',
      field('name', $.identifier),
      '(',
      ')',
      field('body', $.fn_body),
    ),

    inline_fn_decl: $ => seq(
      optional('pub'),
      'inline',
      'fn',
      field('name', $.identifier),
      '(',
      field('params', optional($.param_list)),
      ')',
      field('body', $.fn_body),
    ),

    param_list: $ => sep1($.identifier, ','),

    struct_decl: $ => seq(
      'struct',
      field('name', $.identifier),
      '{',
      field('fields', $.field_list),
      '}',
    ),

    field_list: $ => seq(
      sep1($.field, ','),
      optional(','),
    ),

    field: $ => seq(
      optional('volatile'),
      field('name', $.identifier),
      ':',
      field('type', $._type),
      optional($.array_dim),
    ),

    type_decl: $ => seq(
      'type',
      field('name', $.identifier),
      '=',
      field('type', $._type),
      optional(';'),
    ),

    enum_decl: $ => seq(
      'enum',
      field('name', $.identifier),
      '{',
      field('variants', $.enum_variant_list),
      '}',
    ),

    enum_variant_list: $ => seq(
      sep1($.enum_variant, ','),
      optional(','),
    ),

    enum_variant: $ => seq(
      field('name', $.identifier),
      optional(seq('=', $._expr)),
    ),

    mod_decl: $ => seq(
      optional('pub'),
      'mod',
      field('name', $.identifier),
      choice(
        ';',
        seq('{', repeat($._module_item), '}'),
      ),
    ),

    use_decl: $ => seq(
      optional('pub'),
      'use',
      sep1($.use_tree, ','),
      optional(';'),
    ),

    use_tree: $ => choice(
      $.use_alias,
      $.use_glob,
      $.use_group,
      $.use_simple,
    ),

    use_simple: $ => prec.left(seq(
      $.use_path_root,
      repeat(seq('::', $.identifier)),
    )),

    use_path_root: $ => choice(
      'lib',
      'self',
      'super',
      $.identifier,
    ),

    use_glob: $ => prec.left(1, seq(
      $.use_path_root,
      repeat(seq('::', $.identifier)),
      '::',
      '*',
    )),

    use_group: $ => prec.left(1, seq(
      $.use_path_root,
      repeat(seq('::', $.identifier)),
      '::',
      '{',
      sep1($.use_tree, ','),
      optional(','),
      '}',
    )),

    use_alias: $ => prec.left(seq(
      choice($.use_simple, $.use_glob, $.use_group),
      'as',
      field('alias', $.identifier),
    )),

    // --- Types --------------------------------------------------------------

    _type: $ => choice(
      $.primitive_type,
      $.array_type,
      $.identifier,
    ),

    primitive_type: $ => choice(...PRIMITIVE_TYPES),

    array_type: $ => choice(
      seq('[', $._type, ']'),
      seq('[', $._type, ';', $._expr, ']'),
    ),

    array_dim: $ => seq('[', optional($._expr), ']'),

    // --- Function body ------------------------------------------------------

    fn_body: $ => seq('{', repeat($._fn_stmt), '}'),

    _fn_stmt: $ => choice(
      $.label,
      $.assembly_stmt,
      $.if_stmt,
      $.while_stmt,
      $.do_while_stmt,
      $.loop_stmt,
      $.switch_stmt,
      $.fn_call,
      $.return_stmt,
      $.var_decl,
      $.macro_stmt,
    ),

    label: $ => seq(
      $.label_def,
      choice(
        $.label,
        $.assembly_stmt,
        $.if_stmt,
        $.while_stmt,
        $.do_while_stmt,
        $.loop_stmt,
        $.switch_stmt,
        $.fn_call,
        $.return_stmt,
        $.var_decl,
        $.macro_stmt,
      ),
    ),

    label_def: $ => seq("'", $.identifier, ':'),

    return_stmt: $ => 'return',

    // --- Assembly -----------------------------------------------------------

    // Operands are separated by commas; each operand may carry a trailing
    // comma. The shapes that bind `, index_reg` into the memory operand take
    // precedence over list separation, matching the compiler, where only
    // x, y, cpu::x, and cpu::y continue a memory operand after a comma.
    assembly_stmt: $ => seq($.opcode, repeat(seq($._operand, optional(',')))),

    opcode: $ => choice(...OPCODES),

    _operand: $ => choice(
      $.immediate,
      $.memory_operand,
      $.register_ref,
      $.label_ref,
      $.selector,
      $.path,
    ),

    immediate: $ => seq('#', $._expr),

    memory_operand: $ => choice(
      prec(3, seq(optional($.mode_prefix), $._expr)),
      prec(3, seq(optional($.mode_prefix), '(', $._expr, ')')),
      prec.dynamic(1, prec(3, seq(optional($.mode_prefix), $._expr, ',', $.index_reg))),
      prec(4, seq(optional($.mode_prefix), '(', $._expr, ',', $._index_operand_reg, ')')),
      prec.dynamic(1, prec(3, seq(optional($.mode_prefix), '(', $._expr, ')', ',', $.index_reg))),
      // SM83 register-atom spellings the compiler normalizes: (hl+) and
      // (hl-). The bump spelling is one token, so `hl+` and `hl-` are not
      // extracted as keywords and the word `hl` stays readable as the
      // parenthesized expression in `ld (hl), a`.
      $._hl_inc_dec_atom,
    ),

    _hl_inc_dec_atom: $ => seq('(', token(choice('hl+', 'hl-')), ')'),

    mode_prefix: $ => choice(...MODE_PREFIXES),

    // The `, index_reg` continuation after a statement operand matches the
    // compiler's gated continuation: only x, y, cpu::x, and cpu::y bind into
    // the indexed operand, and any other identifier starts a new operand.
    // Inside a parenthesized memory operand the compiler instead accepts any
    // identifier or cpu::register as the index (parse_index_reg), so that
    // position stays broad.
    index_reg: $ => choice($.index_register, $.cpu_index_reg),

    index_register: $ => choice('x', 'y'),

    cpu_index_reg: $ => seq('cpu', '::', $.index_register),

    _index_operand_reg: $ => choice($.register_ref, $.identifier),

    register_ref: $ => seq('cpu', '::', $.identifier),

    label_ref: $ => seq("'", $.identifier),

    // A selector chains a base identifier through `::` module accesses and
    // `.` field accesses, matching the compiler's selector continuation, so
    // both `a::b` and `a.b` parse with a bare identifier base. The compiler
    // also accepts `+primary`/`-primary` offset continuations on selectors;
    // those spans already parse as additive expressions, so both readings
    // cover the same operand text.
    selector: $ => prec(10, seq(
      $._selector_start,
      repeat1(choice(
        seq('::', $.identifier),
        seq('.', $.identifier),
      )),
    )),

    _selector_start: $ => $.identifier,

    path: $ => prec.left(seq(
      $.identifier,
      repeat1(seq('::', $.identifier)),
    )),

    // --- Control flow -------------------------------------------------------

    if_stmt: $ => prec.right(seq(
      'if',
      '(',
      optional($.branch_hint),
      $.if_condition,
      ')',
      choice($.block, $._fn_stmt),
      optional($.else_block),
    )),

    // Or-chain `if` conditions; each clause may open with a block. The plain
    // `condition` shape stays for while and do-while, matching the compiler.
    if_condition: $ => seq(
      $.condition_clause,
      repeat(seq('or', $.condition_clause)),
    ),

    condition_clause: $ => seq(
      optional($.block),
      $.condition,
    ),

    else_block: $ => seq(
      'else',
      choice($.block, $._fn_stmt),
    ),

    while_stmt: $ => seq(
      'while',
      '(',
      optional($.branch_hint),
      $.condition,
      ')',
      choice($.block, $._fn_stmt),
    ),

    do_while_stmt: $ => seq(
      'do',
      choice($.block, $._fn_stmt),
      'while',
      '(',
      optional($.branch_hint),
      $.condition,
      ')',
    ),

    loop_stmt: $ => seq(
      'loop',
      choice($.block, $._fn_stmt),
    ),

    switch_stmt: $ => seq(
      'switch',
      '(',
      choice($.register_ref, $.identifier),
      ')',
      '{',
      repeat($.switch_case),
      '}',
    ),

    switch_case: $ => choice(
      seq('case', $._expr, choice($.block, $._fn_stmt)),
      seq('default', choice($.block, $._fn_stmt)),
    ),

    branch_hint: $ => choice('near', 'far'),

    condition: $ => seq(
      repeat($.modifier),
      $.condition_keyword,
    ),

    modifier: $ => choice(...CONDITION_MODIFIERS),

    condition_keyword: $ => choice(...CONDITION_KEYWORDS),

    block: $ => seq('{', repeat($._fn_stmt), '}'),

    // --- Function/macro calls ------------------------------------------------

    fn_call: $ => prec(2, seq(
      field('function', $.identifier),
      '(',
      optional($.arg_list),
      ')',
    )),

    arg_list: $ => sep1($._expr, ','),

    // --- Expressions --------------------------------------------------------

    _expr: $ => $.expr,

    expr: $ => $._or_expr,

    _or_expr: $ => prec.left(1, seq(
      $._xor_expr,
      repeat(seq('|', $._xor_expr)),
    )),

    _xor_expr: $ => prec.left(2, seq(
      $._and_expr,
      repeat(seq('^', $._and_expr)),
    )),

    _and_expr: $ => prec.left(3, seq(
      $._eq_expr,
      repeat(seq('&', $._eq_expr)),
    )),

    _eq_expr: $ => prec.left(4, seq(
      $._cmp_expr,
      repeat(seq(choice('==', '!='), $._cmp_expr)),
    )),

    _cmp_expr: $ => prec.left(5, seq(
      $._shift_expr,
      repeat(seq(choice('<', '>', '<=', '>='), $._shift_expr)),
    )),

    _shift_expr: $ => prec.left(6, seq(
      $._add_expr,
      repeat(seq(choice('<<', '>>'), $._add_expr)),
    )),

    _add_expr: $ => prec.left(7, seq(
      $._mul_expr,
      repeat(seq(choice('+', '-'), $._mul_expr)),
    )),

    _mul_expr: $ => prec.left(8, seq(
      $._unary_expr,
      repeat(seq(choice('*', '/', '%'), $._unary_expr)),
    )),

    _unary_expr: $ => prec.left(9, choice(
      seq(choice('~', '!', '-', '+'), $._unary_expr),
      $._primary,
    )),

    _primary: $ => choice(
      $.number,
      $.string,
      'true',
      'false',
      $.selector,
      $.path,
      $.fn_call,
      $.macro_call,
      $.include_macro_call,
      $.array_literal,
      $.struct_literal,
      seq('(', $._expr, ')'),
      $.identifier,
    ),

    // Compile-time macro statements in fn bodies: an optional semicolon,
    // matching the compiler's optional_semicolon.
    macro_stmt: $ => seq($.macro_call, optional(';')),

    // The macro name and the `!` are a single token. The bare names (a `len`
    // parameter or argument, for example) stay ordinary identifiers: the
    // compiler reads them as words, and keyword extraction would otherwise
    // shadow the word token in expression and argument positions.
    macro_call: $ => seq(
      field('macro', $.macro_name),
      '(',
      $._expr,
      ')',
    ),

    macro_name: $ => token(choice(...COMPILE_MACROS.map((name) => name + '!'))),

    include_macro_call: $ => seq(
      field('macro', $.include_macro_name),
      '(',
      choice($.string, $.path, $.selector),
      ')',
    ),

    include_macro_name: $ => token(choice(...INCLUDE_MACROS.map((name) => name + '!'))),

    // Bracket array literals as expression primaries.
    array_literal: $ => prec.left(seq(
      '[',
      optional(seq(
        sep1($._expr, ','),
        optional(','),
      )),
      ']',
    )),

    // Struct literals in expression position: Type { field: expr, ... }. The
    // compiler binds an identifier followed by a brace to a struct literal in
    // every expression position, so this rule takes precedence over the
    // plain identifier reads.
    struct_literal: $ => prec.left(5, seq(
      field('type_name', $.identifier),
      '{',
      optional(seq(
        sep1(seq(field('field', $.identifier), ':', $._expr), ','),
        optional(','),
      )),
      '}',
    )),

    // --- Literals -----------------------------------------------------------

    literal: $ => choice(
      $.number,
      $.string,
      'true',
      'false',
    ),

    number: $ => choice(
      $.decimal_number,
      $.binary_number,
      $.hex_number,
    ),

    decimal_number: $ => token(/[1-9][0-9]*|0/),

    binary_number: $ => token(seq('%', /[01]+/)),

    hex_number: $ => token(seq('0x', /[0-9a-fA-F]+/)),

    string: $ => seq(
      '"',
      repeat(choice(
        $.string_escape,
        /[^"\\]+/,
      )),
      '"',
    ),

    string_escape: $ => token.immediate(/\\[nrt0a\\"]/),

    // --- Comments -----------------------------------------------------------

    line_comment: $ => token(prec(-1, seq('//', /.*/))),

    block_comment: $ => token(seq('/*', repeat(choice(/[^*]/, /\*[^/]/)), '*/')),

    doc_comment: $ => token(seq('///', /.*/)),

    module_doc_comment: $ => token(seq('//!', /.*/)),

    // --- Identifiers --------------------------------------------------------

    identifier: $ => /[A-Za-z_][A-Za-z0-9_]*/,
  },
});

// Join the given rule with a separator. Produces a sequence of one or more
// `rule` elements separated by `sep`.
// @param {Rule} rule
// @param {string|Rule} sep
// @returns {Rule}
function sep1(rule, sep) {
  return seq(rule, repeat(seq(sep, rule)));
}