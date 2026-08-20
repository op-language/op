" Op syntax highlighting for Vim/Neovim (fallback when tree-sitter is absent).
" Based on docs/language-specification.md.
"
" This file defines syntax highlighting for the Op language as specified
" in the Op language specification. It is the non-tree-sitter fallback.
" The tree-sitter grammar in tree-sitter-op/ is the primary highlighter.

if exists("b:current_syntax")
  finish
endif

" --- Keywords ---------------------------------------------------------------

syntax keyword opKeyword fn inline return noreturn volatile
syntax keyword opKeyword struct type enum const mod use pub lib self super
syntax keyword opKeyword if else while do loop switch case default
syntax keyword opKeyword near far as
syntax keyword opBoolean true false

" --- Compile-time macros ----------------------------------------------------
" lo!(), hi!(), nylo!(), nyhi!(), sizeof!(), len!()
" These use Rust-style macro call syntax with the trailing !.

syntax match opBuiltin "\<\(lo\|hi\|nylo\|nyhi\|sizeof\|len\)!"

" --- File inclusion macros --------------------------------------------------

syntax keyword opMacro locate_bytes locate_str locate_fn
syntax keyword opMacro include_bytes include_str include_fn

" --- Control-flow condition keywords ---------------------------------------
" These appear inside if/while/do-while condition parentheses.
" They are target-specific but the common 6502 family set is included.

syntax keyword opCondition plus positive minus negative greater less
syntax keyword opCondition overflow carry nonzero set zero unset
syntax keyword opCondition false clear equal
syntax keyword opCondition is has no not

" --- Addressing-mode prefixes ----------------------------------------------

syntax keyword opModePrefix zp abs rel ind idx ind_l ind_idx

" --- Types ------------------------------------------------------------------

syntax keyword opType u8 i8 u16 i16 u32 i32 bool pointer

" --- Numbers ----------------------------------------------------------------

" Decimal integers
syntax match opNumber "\<\([1-9][0-9]*\|0\)\>"

" Binary literals: %10101010
syntax match opNumber "%[01]\+"

" C-style hexadecimal: 0xFF, 0x2000
syntax match opNumber "0x[0-9a-fA-F]\+"

" --- Strings ----------------------------------------------------------------

syntax region opString start=+"+ skip=+\\\\\|\\"+ end=+"+ contains=opStringEscape
syntax match opStringEscape "\\[nrt0a\\\"]" contained

" --- Attributes -------------------------------------------------------------

syntax match opAttribute "#\[[^]]*\]" contains=opAttributeInner
syntax region opAttributeInner start="#\[" end="\]" contained contains=opAttributeName,opString,opNumber,opBoolean
syntax keyword opAttributeName cfg interrupt addr rom ram chr align setpad ines lnx loader contained

" --- Immediate operands -----------------------------------------------------

syntax match opImmediate "#"

" --- Operators --------------------------------------------------------------

syntax match opOperator "::"
syntax match opOperator "<<"
syntax match opOperator ">>"
syntax match opOperator ">="
syntax match opOperator "<="
syntax match opOperator "=="
syntax match opOperator "!="
syntax match opOperator "[-+*/%~!&^|<>=.,;:(){}\[\]]"

" --- Labels -----------------------------------------------------------------
" A label is 'identifier: followed by a statement on the same line.

syntax match opLabel "'[a-zA-Z_][a-zA-Z0-9_]*:"

" --- 6502 opcodes (common set) ---------------------------------------------
" These are target-specific but the 6502 family is the primary target.

syntax keyword opOpcode adc and asl bcc bcs beq bit bmi bne bpl brk bvc bvs
syntax keyword opOpcode clc cld cli clv cmp cpx cpy dec dex dey eor inc inx
syntax keyword opOpcode iny jmp jsr lda ldx ldy lsr nop ora pha php pla plp
syntax keyword opOpcode rol ror rti rts sbc sec sed sei sta stx sty tax tay
syntax keyword opOpcode tsx txa txs tya

" 65SC02 / 65C816 additions
syntax keyword opOpcode bra phx phy plx ply stz tsb trb ina dea

" 65C816 additions
syntax keyword opOpcode rep sep xba xce tcd tdc tcs tsc txy tyx mvn mvp
syntax keyword opOpcode pea pei per jml jsl rtl cop wai stp

" 68000 mnemonics
syntax keyword opOpcode move moveq movem lea clr not and or eor add adda
syntax keyword opOpcode addi addq sub suba subi subq mulu muls divu divs neg
syntax keyword opOpcode negx abs asl asr lsl lsr rol ror roxl roxr cmp cmpa
syntax keyword opOpcode cmpi tst btst bset bclr bchg rts rtr rte bcc bsr dbcc
syntax keyword opOpcode chk trap trapv swap exg ext link unlk reset nop stop
syntax keyword opOpcode illegal

" Z80 mnemonics
syntax keyword opOpcode ld push pop ex exx ldi ldir ldd lddr cpi cpir cpd cpdr
syntax keyword opOpcode add adc sub sbc cp inc dec daa cpl neg ccf scf nop
syntax keyword opOpcode halt di ei im rlc rl rrc rr sla sra sll srl rld rrd
syntax keyword opOpcode rlca rla rrca rra jp jr djnz call ret reti retn rst
syntax keyword opOpcode in out ini inir ind indr outi otir outd otdr bit set res

" --- Register references (cpu::X) -------------------------------------------

syntax match opRegister "cpu::[a-zA-Z_][a-zA-Z0-9_]*"

" --- Comments ---------------------------------------------------------------
" Comments are defined LAST so that they win over operators and macro bang
" matches at the same position.  In Vim, when multiple syntax items start at
" the same position, the one defined last wins.  This prevents // from being
" split into two / operators, and prevents //! from having the ! eaten by
" the operator or macro-bang match.
"
" Doc comment regions are defined after plain comments so they win at the
" same start position, preventing /// and //! from being consumed by
" the plain // comment match.

syntax match opComment "//.*$" contains=opTodo
syntax region opComment start="/\*" end="\*/" contains=opTodo
syntax region opModuleDocComment start="//!" end="$" contains=opTodo
syntax region opDocComment start="///" end="$" contains=opTodo
syntax keyword opTodo TODO FIXME HACK NOTE XXX contained

" --- Highlight links --------------------------------------------------------

highlight default link opKeyword            Keyword
highlight default link opBoolean            Boolean
highlight default link opBuiltin            Function
highlight default link opMacro              Macro
highlight default link opCondition          Conditional
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
highlight default link opOpcode             Keyword
highlight default link opRegister           Special

let b:current_syntax = "op"