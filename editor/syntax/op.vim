" HLA syntax highlighting for Neovim
" Based on docs/language-specification.md
"
" This file defines syntax highlighting for the HLA language
" as specified in the hlacc language specification.

if exists("b:current_syntax")
  finish
endif

" --- Keywords ---------------------------------------------------------------

syntax keyword hlaKeyword fn inline return noreturn volatile
syntax keyword hlaKeyword struct type enum const mod use pub
syntax keyword hlaKeyword if else while do loop switch case default
syntax keyword hlaKeyword near far as
syntax keyword hlaBoolean true false

" --- Compile-time macros ----------------------------------------------------
" lo!(), hi!(), nylo!(), nyhi!(), sizeof!()
" These use Rust-style macro call syntax with the trailing !.
" Match the name plus the ! as a single token.

syntax match hlaBuiltin "\<\(lo\|hi\|nylo\|nyhi\|sizeof\)!"

" --- Control-flow condition keywords ---------------------------------------
" These appear inside if/while/do-while condition parentheses.
" They are target-specific but the common 6502 family set is included.

syntax keyword hlaCondition plus positive minus negative greater less
syntax keyword hlaCondition overflow carry nonzero set true zero unset
syntax keyword hlaCondition false clear equal
syntax keyword hlaCondition is has no not

" --- Types ------------------------------------------------------------------

syntax keyword hlaType u8 i8 u16 i16 u32 i32 bool pointer

" --- Numbers ----------------------------------------------------------------

" Decimal integers
syntax match hlaNumber "\<\([1-9][0-9]*\|0\)\>"

" Binary literals: %10101010
syntax match hlaNumber "%[01]\+"

" C-style hexadecimal: 0xFF, 0x2000
syntax match hlaNumber "0x[0-9a-fA-F]\+"

" --- Strings ----------------------------------------------------------------

syntax region hlaString start=+"+ skip=+\\\\\|\\"+ end=+"+ contains=hlaStringEscape
syntax match hlaStringEscape "\\[nrt0a\\\"]" contained

" --- Attributes -------------------------------------------------------------

syntax match hlaAttribute "#\[[^]]*\]" contains=hlaAttributeInner
syntax region hlaAttributeInner start="#\[" end="\]" contained contains=hlaAttributeName,hlaString,hlaNumber,hlaBoolean
syntax keyword hlaAttributeName cfg interrupt addr rom ram chr align setpad ines lnx loader contained

" --- Immediate operands -----------------------------------------------------

syntax match hlaImmediate "#"

" --- Operators --------------------------------------------------------------

syntax match hlaOperator "::"
syntax match hlaOperator "->"
syntax match hlaOperator "<<"
syntax match hlaOperator ">>"
syntax match hlaOperator ">="
syntax match hlaOperator "<="
syntax match hlaOperator "=="
syntax match hlaOperator "!="
syntax match hlaOperator "&&"
syntax match hlaOperator "||"
syntax match hlaOperator "[-+*/%~!&^|<>=.,;:(){}\[\]]"

" --- Labels -----------------------------------------------------------------
" A label is 'identifier: followed by a statement on the same line.
" The label itself is just the 'identifier: part.

syntax match hlaLabel "'[a-zA-Z_][a-zA-Z0-9_]*:"

" --- Module include macros --------------------------------------------------
" include_bytes!(), include_str!(), include_fn!()

syntax keyword hlaMacro include_bytes include_str include_fn

" --- 6502 opcodes (common set) ---------------------------------------------
" These are target-specific but the 6502 family is the primary target.
" Additional opcode sets can be added per-target.

syntax keyword hlaOpcode adc and asl bcc bcs beq bit bmi bne bpl brk bvc bvs
syntax keyword hlaOpcode clc cld cli clv cmp cpx cpy dec dex dey eor inc inx
syntax keyword hlaOpcode iny jmp jsr lda ldx ldy lsr nop ora pha php pla plp
syntax keyword hlaOpcode rol ror rti rts sbc sec sed sei sta stx sty tax tay
syntax keyword hlaOpcode tsx txa txs tya

" 65SC02 / 65C816 additions
syntax keyword hlaOpcode bra phx phy plx ply stz tsb trb ina dea

" 65C816 additions
syntax keyword hlaOpcode rep sep xba xce tcd tdc tcs tsc txy tyx mvn mvp
syntax keyword hlaOpcode pea pei per jml jsl rtl cop wai stp

" 68000 mnemonics
syntax keyword hlaOpcode move moveq movem lea pea clr not and or eor add adda
syntax keyword hlaOpcode addi addq sub suba subi subq mulu muls divu divs neg
syntax keyword hlaOpcode negx abs asl asr lsl lsr rol ror roxl roxr cmp cmpa
syntax keyword hlaOpcode cmpi tst btst bset bclr bchg jmp jsr rts rtr rte bcc
syntax keyword hlaOpcode bra bsr dbcc chk trap trapv swap exg ext link unlk
syntax keyword hlaOpcode reset nop stop illegal

" Z80 mnemonics
syntax keyword hlaOpcode ld push pop ex exx ldi ldir ldd lddr cpi cpir cpd cpdr
syntax keyword hlaOpcode add adc sub sbc cp inc dec daa cpl neg ccf scf nop
syntax keyword hlaOpcode halt di ei im rlc rl rrc rr sla sra sll srl rld rrd
syntax keyword hlaOpcode rlca rla rrca rra jp jr djnz call ret reti retn rst
syntax keyword hlaOpcode in out ini inir ind indr outi otir outd otdr bit set res

" --- Register references (cpu::X) -------------------------------------------

syntax match hlaRegister "cpu::[a-zA-Z_][a-zA-Z0-9_]*"

" --- Comments ---------------------------------------------------------------
" Comments are defined LAST so that they win over operators and macro bang
" matches at the same position.  In Vim, when multiple syntax items start at
" the same position, the one defined last wins.  This prevents // from being
" split into two / operators, and prevents //! from having the ! eaten by
" the operator or macro-bang match.
"
" Doc comment regions are defined after plain comments so they win at the
" same start position, preventing /// and //! from being consumed by the
" plain // comment match.

syntax match hlaComment "//.*$" contains=hlaTodo
syntax region hlaComment start="/\*" end="\*/" contains=hlaTodo
syntax region hlaModuleDocComment start="//!" end="$" contains=hlaTodo
syntax region hlaDocComment start="///" end="$" contains=hlaTodo
syntax keyword hlaTodo TODO FIXME HACK NOTE XXX contained

" --- Highlight links --------------------------------------------------------

highlight default link hlaKeyword      Keyword
highlight default link hlaBoolean      Boolean
highlight default link hlaBuiltin      Function
highlight default link hlaCondition    Conditional
highlight default link hlaType         Type
highlight default link hlaComment      Comment
highlight default link hlaDocComment   SpecialComment
highlight default link hlaModuleDocComment SpecialComment
highlight default link hlaTodo         Todo
highlight default link hlaNumber       Number
highlight default link hlaString       String
highlight default link hlaStringEscape SpecialChar
highlight default link hlaAttribute    PreProc
highlight default link hlaAttributeName PreProc
highlight default link hlaImmediate    Special
highlight default link hlaOperator     Operator
highlight default link hlaLabel        Label
highlight default link hlaMacro        Macro
highlight default link hlaOpcode       Keyword
highlight default link hlaRegister     Special

let b:current_syntax = "hla"