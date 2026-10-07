" Op syntax highlighting for Vim/Neovim (fallback when tree-sitter is absent).
" The tree-sitter grammar in editor/tree-sitter-op/ is the primary
" highlighter; this file is the self-contained regex fallback. It covers the
" full token surface of the current compiler: all language keywords, types,
" the condition vocabulary, addressing-mode prefixes, compile-time and
" include macros, attributes, labels, and every machine-specific CPU opcode
" of the 6502, 6502 undocumented set, 65SC02, W65C02, 65C816, 68000, Z80,
" and SM83/LR35902 families. It needs no build step and no parser binary.
"
" License: Apache-2.0 (see the LICENSE file at the repository root).
"
" Source of truth for the vocabulary:
"   - crates/opc/src/lexer.rs: the opcode strings in the OPCODES const
"     (lines 143 to 177), the classification tables (lines 21 to 138) for
"     keywords, types, condition words, mode prefixes, and macro names, and
"     the STRING_ESCAPES const (line 219) for the string escape set. The
"     lexer folds opcode words to lowercase; the other word tables compare
"     case-sensitively.
"   - std/src/cpu/*.op: the OPCODE enums of the CPU library modules. The
"     W65C816 library declares nine mnemonics the lexer does not yet tokenize
"     as opcodes (phb phd phk plb pld tad tda tsa wdm); they are highlighted
"     anyway so standard library sources read cleanly.
"   - docs/language-specification.md: the sections on comments, attributes,
"     assembly statements, conditions, and labels.
"
" Definition-order rules used below, as observed in Neovim syntax
" resolution: an item that starts earlier on the line wins before items that
" start later; at the same start position a keyword wins over a pattern
" item, and between pattern items the later-defined one wins. Several items
" are deliberately ordered to use these rules (numbers after operators,
" attributes after the immediate, labels and comments late).

if exists("b:current_syntax")
  finish
endif

" The lexer folds only opcode words to lowercase; the other word tables
" compare case-sensitively. This file matches every spelling
" case-insensitively, so it also colors a few forms the compiler would
" reject, for example an uppercase 0X hex prefix as a number. Cosmetic only.
syn case ignore

" --- Keywords ---------------------------------------------------------------

syntax keyword opKeyword fn inline return noreturn volatile
syntax keyword opKeyword struct type enum const mod use pub lib self super
syntax keyword opKeyword if else while do loop switch case default
syntax keyword opKeyword near far as
syntax keyword opBoolean true false

" --- Types ------------------------------------------------------------------

syntax keyword opType u8 i8 u16 i16 u32 i32 bool pointer

" --- Compile-time macros ----------------------------------------------------
" lo!(), hi!(), nylo!(), nyhi!(), sizeof!(), len!(), and the assertion
" macros compile_error!(), assert!(), assert_eq!(), debug_assert!(),
" debug_assert_eq!(), panic!(). The lexer classifies these names only when
" the trailing bang is present, so the pattern requires it.

syntax match opCompileMacro "\<\(compile_error\|debug_assert_eq\|debug_assert\|assert_eq\|assert\|panic\|nylo\|nyhi\|sizeof\|lo\|hi\|len\)!"

" --- File inclusion macros --------------------------------------------------
" locate_str! and font_load! create placement items.

syntax match opIncludeMacro "\<\(locate_str\|font_load\)!"

" --- Control-flow condition keywords ---------------------------------------
" The full 31-word condition vocabulary of the lexer, used inside
" if/while/do-while condition parentheses. true and false are keywords to
" the lexer, never condition tokens; they stay in the boolean group.

syntax keyword opCondition plus positive minus negative greater less
syntax keyword opCondition overflow carry nonzero set zero unset clear equal
syntax keyword opCondition high low_or_same carry_clear carry_set not_equal
syntax keyword opCondition overflow_clear overflow_set greater_or_equal
syntax keyword opCondition less_than greater_than less_or_equal not_zero
syntax keyword opCondition no_carry parity_even parity_odd sign_positive
syntax keyword opCondition sign_negative

" Condition modifiers.
syntax keyword opModifier is has no not

" --- Addressing-mode prefixes ----------------------------------------------

syntax keyword opModePrefix zp abs rel ind idx ind_l ind_idx

" --- Operators --------------------------------------------------------------
" Braces and brackets deliberately stay in the operator set, which also
" colors array literals [ ... ] and struct literals Type { ... }.
" Defined before the number and attribute items so those items win the
" collisions below (binary literal %, attribute #\[).

syntax match opOperator "::"
syntax match opOperator "<<"
syntax match opOperator ">>"
syntax match opOperator ">="
syntax match opOperator "<="
syntax match opOperator "=="
syntax match opOperator "!="
syntax match opOperator "[-+*/%~!&^|<>=.,;:(){}\[\]]"

" --- Numbers ----------------------------------------------------------------
" Defined after the operator items: both matches can start at the % of a
" binary literal, and the later-defined one wins, so %10101010 keeps its
" number color while a bare % keeps the operator color.

" Decimal integers.
syntax match opNumber "\<\([1-9][0-9]*\|0\)\>"

" Binary literals: %10101010. The lexer treats % as a binary prefix only
" when digits follow.
syntax match opNumber "%[01]\+"

" Hexadecimal literals: 0x2000. The lexer accepts a lowercase x only.
syntax match opNumber "0x[0-9a-fA-F]\+"

" --- Immediate operands -----------------------------------------------------
" Defined before the attribute items for the same winner rule: the # of
" #[...] starts the attribute region, a bare # stays an immediate prefix.

syntax match opImmediate "#"

" --- Attributes -------------------------------------------------------------
" Module and declaration attributes. The generic bracket region colors the
" whole attribute, including combinator payloads such as
" #[cfg(all(cpu = "rp2A03", any(feature = "a")))] and dotted keys like
" ines.mapper. The names addr, align, and loader are declared in the
" language specification but not yet consumed by the compiler.

syntax match opAttribute "#\[[^]]*\]" contains=opAttributeInner
" The inner region starts at the bracket, one character after the outer
" match, so the outer opAttribute stays visible at the # and colors the
" attribute prefix.
syntax region opAttributeInner start="\[" end="\]" contained contains=opAttributeName,opString,opNumber,opBoolean
syntax keyword opAttributeName cfg crash_handler interrupt addr rom ram chr align setpad ines lnx loader gb snes sega sms a78 crt locate contained

" --- Strings ----------------------------------------------------------------

syntax region opString start=+"+ skip=+\\\\\|\\"+ end=+"+ contains=opStringEscape
syntax match opStringEscape "\\[nrt0a\\\"]" contained

" --- Labels -----------------------------------------------------------------
" A label definition is 'name: followed by a statement; a reference is 'name
" without the trailing colon. The reference match is defined BEFORE the
" definition match: both start at the same character, and the longer
" definition match wins when a colon follows. Without the colon only the
" reference matches.

syntax match opLabelRef "'[a-zA-Z_][a-zA-Z0-9_]*"
syntax match opLabel "'[a-zA-Z_][a-zA-Z0-9_]*:"

" --- Register references (cpu::X) -------------------------------------------

syntax match opRegister "cpu::[a-zA-Z_][a-zA-Z0-9_]*"

" --- Opcodes ----------------------------------------------------------------
" The machine-specific opcode vocabulary of the lexer, one keyword set per
" CPU family. A few words appear in more than one family; the repeats are
" harmless and keep every family listing readable on its own.
"
" Dual-membership words: or doubles as the or-chain separator inside if
" conditions, and abs (mode prefix), ind (mode prefix), not (condition
" modifier), and set (condition keyword) double as 68000/Z80 mnemonics.
" These all show the opcode color, since the opcode groups are defined
" after the keyword groups above. Purely cosmetic.

" 6502 core
syntax keyword opOpcode adc and asl bcc bcs beq bit bmi bne bpl brk bvc bvs
syntax keyword opOpcode clc cld cli clv cmp cpx cpy dec dex dey eor inc inx
syntax keyword opOpcode iny jmp jsr lda ldx ldy lsr nop ora pha php pla plp
syntax keyword opOpcode rol ror rti rts sbc sec sed sei sta stx sty tax tay
syntax keyword opOpcode tsx txa txs tya

" 6502 undocumented instructions (tas is also the 65C816 TAS)
syntax keyword opOpcode alr anc ane arr dcp isc las lax lxa rla rra sax sha
syntax keyword opOpcode shx shy slo sre tas usbc

" 65SC02 additions
syntax keyword opOpcode bra phx phy plx ply stz tsb trb ina dea

" W65C02 Rockwell bit operations (wai and stp are shared with the 65C816)
syntax keyword opOpcode rmb0 rmb1 rmb2 rmb3 rmb4 rmb5 rmb6 rmb7
syntax keyword opOpcode smb0 smb1 smb2 smb3 smb4 smb5 smb6 smb7
syntax keyword opOpcode bbr0 bbr1 bbr2 bbr3 bbr4 bbr5 bbr6 bbr7
syntax keyword opOpcode bbs0 bbs1 bbs2 bbs3 bbs4 bbs5 bbs6 bbs7

" 65C816
syntax keyword opOpcode rep sep xba xce tcd tdc tcs tsc txy tyx mvn mvp
syntax keyword opOpcode pea pei per jml jsl rtl cop wai stp

" W65C816 mnemonics declared by the std CPU library only, not yet listed in
" the lexer; highlighted so standard library sources read cleanly.
syntax keyword opOpcode phb phd phk plb pld tad tda tsa wdm

" 68000 (jmp jsr rts bra asl lsr rol ror and cmp also exist in the
" 6502-family groups above)
syntax keyword opOpcode move moveq movem lea clr not or eor add adda addi
syntax keyword opOpcode addq sub suba subi subq mulu muls divu divs neg negx
syntax keyword opOpcode abs asr lsl lsr ror roxl roxr cmpa cmpi tst btst
syntax keyword opOpcode bset bclr bchg rtr rte bsr dbcc chk trap trapv swap
syntax keyword opOpcode exg ext link unlk reset stop illegal

" Z80 (sbc inc dec and rra repeat the 6502-family lists, exactly as the
" lexer lists them)
syntax keyword opOpcode ld push pop ex exx ldi ldir ldd lddr cpi cpir cpd
syntax keyword opOpcode cpdr sbc cp inc dec daa cpl ccf scf halt di ei im
syntax keyword opOpcode rlc rl rrc rr sla sra sll srl rld rrd rlca rrca
syntax keyword opOpcode rra jp jr djnz call ret reti retn rst in out ini
syntax keyword opOpcode inir ind indr outi otir outd otdr bit set res xor

" SM83 additions (stop is shared with the 68000)
syntax keyword opOpcode stop ldh

" SM83 underscore register-pair pseudo-instructions
syntax keyword opOpcode inc_hl inc_de inc_bc ld_hl ld_de ld_bc ld_a_hl
syntax keyword opOpcode ld_a_bc ld_a_de ld_ba ld_ca ld_da ld_ea ld_ha ld_la
syntax keyword opOpcode ld_ab ld_ac ld_ad ld_ae ld_ah ld_al ld_a ld_addr ld_sp
syntax keyword opOpcode ld_hl_a ld_hld_a ld_c_a ld_b ld_c ld_d ld_e ld_h ld_l
syntax keyword opOpcode inc_b inc_c inc_d inc_e inc_h inc_l inc_a
syntax keyword opOpcode dec_b dec_c dec_d dec_e dec_h dec_l dec_a
syntax keyword opOpcode add_a_hl sub_b xor_a bit_h7 rl_c cp_hl
syntax keyword opOpcode jr_nz jr_z jr_nc jr_c

" --- Comments ---------------------------------------------------------------
" Comment items are defined LAST so that they win over operators and macro
" bang matches at the same position (the winner rule above prevents // from
" being split into two / operators, and keeps //! from losing the ! to an
" operator or macro-bang match).
"
" The doc comment items are defined after the plain comment items so they win
" at the same start position (//, ///, and //! all match to end of line).
" The tree-sitter grammar keeps separate doc-comment nodes, so the
" highlighter mirrors that distinction; the compiler itself lexes doc forms
" as plain comments.

syntax match opComment "//.*$" contains=opTodo
syntax region opComment start="/\*" end="\*/" contains=opTodo
syntax region opModuleDocComment start="//!" end="$" contains=opTodo
syntax region opDocComment start="///" end="$" contains=opTodo
syntax keyword opTodo TODO FIXME HACK NOTE XXX contained

" --- Highlight links --------------------------------------------------------

highlight default link opKeyword            Keyword
highlight default link opBoolean            Boolean
highlight default link opCompileMacro       Function
highlight default link opIncludeMacro       Macro
highlight default link opCondition          Conditional
highlight default link opModifier           Conditional
highlight default link opModePrefix         Keyword
highlight default link opType               Type
highlight default link opComment            Comment
highlight default link opDocComment         SpecialComment
highlight default link opModuleDocComment   SpecialComment
highlight default link opTodo               Todo
highlight default link opNumber             Number
highlight default link opString             String
highlight default link opStringEscape       SpecialChar
highlight default link opAttribute          PreProc
highlight default link opAttributeName      PreProc
highlight default link opImmediate          Special
highlight default link opOperator           Operator
highlight default link opLabel              Label
highlight default link opLabelRef           Label
highlight default link opOpcode             Keyword
highlight default link opRegister           Special

let b:current_syntax = "op"