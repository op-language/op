// Op grammar for tree-sitter
//
// Models the normalized LR(1) grammar in
// `op/docs/language-specification.md` (lines 1552-1729).
//
// Op is a high-level assembler for retro game consoles. Source files use
// the `.op` extension. The lexer classifies a statement-leading identifier
// as an opcode if it matches the hard-coded CPU-family keyword set. Libs
// may define further opcodes; unknown statement-leading identifiers fall
// back to a generic `identifier` node so the grammar still parses.

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

const OPCODE_65C816 = [
  'rep', 'sep', 'xba', 'xce', 'tcd', 'tdc', 'tcs', 'tsc', 'txy', 'tyx', 'mvn',
  'mvp', 'pea', 'pei', 'per', 'jml', 'jsl', 'rtl', 'cop', 'wai', 'stp',
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
  'indr', 'outi', 'otir', 'outd', 'otdr', 'bit', 'set', 'res',
];

const OPCODE_LR35902 = [
  'stop', 'ldi', 'ldd', 'ldh',
];

const OPCODES = [
  ...OPCODE_6502,
  ...OPCODE_65SC02,
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
  // shared
  'true', 'false',
];

const CONDITION_MODIFIERS = ['is', 'has', 'no', 'not'];

const KEYWORDS = [
  'fn', 'inline', 'noreturn', 'return', 'volatile', 'struct', 'type', 'enum',
  'const', 'mod', 'use', 'pub', 'if', 'else', 'while', 'do', 'loop', 'switch',
  'case', 'default', 'near', 'far', 'as',
];

const PRIMITIVE_TYPES = [
  'u8', 'i8', 'u16', 'i16', 'u32', 'i32', 'bool', 'pointer',
];

const MODE_PREFIXES = ['zp', 'abs', 'rel', 'ind', 'idx', 'ind_l', 'ind_idx'];

const ATTR_NAMES = [
  'cfg', 'interrupt', 'addr', 'rom', 'ram', 'chr', 'align', 'setpad', 'ines',
  'lnx', 'loader',
];

const COMPILE_MACROS = ['lo', 'hi', 'nylo', 'nyhi', 'sizeof'];

const INCLUDE_MACROS = [
  'locate_bytes', 'locate_str', 'locate_fn',
  'include_bytes', 'include_str', 'include_fn',
];

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
    [$.path, $._primary],
    [$.assembly_stmt, $.assembly_stmt],
    [$.init_list, $._primary],
    [$._operand, $._primary],
    [$.memory_operand, $._primary],
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
      seq($.identifier, '=', $.literal),
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
      'const',
      field('name', $.identifier),
      ':',
      field('type', $._type),
      '=',
      field('value', $._expr),
      optional(';'),
    ),

    var_decl: $ => seq(
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
      optional('noreturn'),
      'fn',
      field('name', $.identifier),
      '(',
      ')',
      field('body', $.fn_body),
    ),

    inline_fn_decl: $ => seq(
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
      'mod',
      field('name', $.identifier),
      choice(
        ';',
        seq('{', repeat($._module_item), '}'),
      ),
    ),

    use_decl: $ => seq(
      'use',
      sep1($.use_path, ','),
      optional(';'),
    ),

    use_path: $ => choice(
      $.use_glob,
      $.use_group,
      $.use_simple,
    ),

    use_simple: $ => prec.left(seq(
      $.identifier,
      repeat(seq('::', $.identifier)),
    )),

    use_glob: $ => seq($.use_simple, '::', '*'),

    use_group: $ => seq(
      $.use_simple,
      '::',
      '{',
      sep1($.identifier, ','),
      optional(','),
      '}',
    ),

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
    ),

    label: $ => seq(
      $.label_def,
      choice(
        $.assembly_stmt,
        $.if_stmt,
        $.while_stmt,
        $.do_while_stmt,
        $.loop_stmt,
        $.switch_stmt,
        $.fn_call,
        $.return_stmt,
        $.var_decl,
      ),
    ),

    label_def: $ => seq("'", $.identifier, ':'),

    return_stmt: $ => 'return',

    // --- Assembly -----------------------------------------------------------

    assembly_stmt: $ => seq(
      $.opcode,
      repeat($._operand),
    ),

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

    memory_operand: $ => prec(3, choice(
      seq(optional($.mode_prefix), $._expr),
      seq(optional($.mode_prefix), '(', $._expr, ')', optional($.index_reg)),
      seq(optional($.mode_prefix), $._expr, ',', $.index_reg),
      seq(optional($.mode_prefix), '(', $._expr, ',', $.index_reg, ')'),
      seq(optional($.mode_prefix), '(', $._expr, ')', ',', $.index_reg),
    )),

    mode_prefix: $ => choice(...MODE_PREFIXES),

    index_reg: $ => choice($.register_ref, $.identifier),

    register_ref: $ => seq('cpu', '::', $.identifier),

    label_ref: $ => seq("'", $.identifier),

    selector: $ => prec(10, seq(
      $.path,
      repeat1(choice(
        seq('::', $.identifier),
        seq('.', $.identifier),
        seq(choice('+', '-'), $._expr),
      )),
    )),

    path: $ => prec.left(seq(
      $.identifier,
      repeat1(seq('::', $.identifier)),
    )),

    // --- Control flow -------------------------------------------------------

    if_stmt: $ => prec.right(seq(
      'if',
      '(',
      optional($.branch_hint),
      $.condition,
      ')',
      choice($.block, $._fn_stmt),
      optional($.else_block),
    )),

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
      $.register_ref,
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
      seq('(', $._expr, ')'),
      $.identifier,
    ),

    macro_call: $ => seq(
      field('macro', choice(...COMPILE_MACROS)),
      '!',
      '(',
      $._expr,
      ')',
    ),

    include_macro_call: $ => seq(
      field('macro', choice(...INCLUDE_MACROS)),
      '!',
      '(',
      choice($.string, $.path, $.selector),
      ')',
    ),

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