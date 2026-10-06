" Risclet follows the case-sensitive, per-line assembler tokenizer and parser.
if exists("b:current_syntax")
  finish
endif
syntax case match

" Generic tokens come first; complete literals and statement context override them.
syntax match riscletError /\v[^ \t\r]/
syntax match riscletSymbol /\v[A-Za-z_$][[:alnum:]_.$]*/
syntax match riscletOperator /\v\<\<|\>\>|[+*/%\&|^~\-]/
syntax match riscletPunctuation /[,():]/
syntax match riscletAddress /\./
syntax match riscletDirectiveError /\v\.[[:alnum:]][[:alnum:]_.$]*/
syntax match riscletRegister /\v[[:alnum:]_.$]@<!%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!/
syntax match riscletNumber /\v%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)/
syntax match riscletNumericError /\v0%([xX][0-9a-fA-F]@!|[bB][01]@!|[oO][0-7]@!)/
syntax match riscletReference /\v%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)[ \t]*[fb][[:alnum:]_.$]@!/

" Quoted tokens stop on the physical line, and only assembler escapes are valid.
syntax match riscletCharacterError /'[^\r\n]*/
syntax match riscletCharacter /\v'%([^'\\\r\n]|\\[ntr\\'"0]|')'/
syntax match riscletStringError /"[^\r\n]*/
syntax region riscletString start=/"/ skip=/\\./ end=/"/ oneline contains=riscletEscape,riscletEscapeError
syntax match riscletEscapeError /\\./ contained
syntax match riscletEscape /\\[ntr\\'"0]/ contained

" A single non-register label can precede an opcode or directive.
syntax match riscletStatementError /\v%(^[ \t]*%(%(%(%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!)@![A-Za-z_$][[:alnum:]_.$]*|%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)|'%([^'\\\r\n]|\\[ntr\\'"0]|')')[ \t]*:[ \t]*)?)@<=%([A-Za-z_$][[:alnum:]_.$]*|\.[[:alnum:]_.$]+)/
syntax match riscletInstruction /\v%(^[ \t]*%(%(%(%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!)@![A-Za-z_$][[:alnum:]_.$]*|%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)|'%([^'\\\r\n]|\\[ntr\\'"0]|')')[ \t]*:[ \t]*)?)@<=%(add|sub|sll|slt|sltu|xor|srl|sra|or|and|mul|mulh|mulhsu|mulhu|div|divu|rem|remu|addi|slli|slti|sltiu|xori|ori|andi|srli|srai|jalr|beq|bne|blt|bge|bltu|bgeu|bgez|bnez|lui|auipc|jal|fence|fence\.tso|fence\.i|ecall|ebreak|lb|lh|lw|lbu|lhu|sb|sh|sw|li|la|call|tail|mv|ret|nop|neg|seqz|snez|sltz|sgtz|beqz|blez|bltz|bgtz|bgt|ble|bgtu|bleu|j|jr|not|c\.add|c\.mv|c\.jr|c\.jalr|c\.li|c\.lui|c\.addi|c\.addi16sp|c\.addi4spn|c\.slli|c\.lwsp|c\.swsp|c\.lw|c\.sw|c\.and|c\.or|c\.xor|c\.sub|c\.srli|c\.srai|c\.andi|c\.beqz|c\.bnez|c\.j|c\.jal|c\.nop|c\.ebreak|lr\.w|lr\.w\.aq|lr\.w\.rel|lr\.w\.aqrl|sc\.w|sc\.w\.aq|sc\.w\.rel|sc\.w\.aqrl|amoswap\.w|amoswap\.w\.aq|amoswap\.w\.rel|amoswap\.w\.aqrl|amoadd\.w|amoadd\.w\.aq|amoadd\.w\.rel|amoadd\.w\.aqrl|amoxor\.w|amoxor\.w\.aq|amoxor\.w\.rel|amoxor\.w\.aqrl|amoand\.w|amoand\.w\.aq|amoand\.w\.rel|amoand\.w\.aqrl|amoor\.w|amoor\.w\.aq|amoor\.w\.rel|amoor\.w\.aqrl|amomin\.w|amomin\.w\.aq|amomin\.w\.rel|amomin\.w\.aqrl|amomax\.w|amomax\.w\.aq|amomax\.w\.rel|amomax\.w\.aqrl|amominu\.w|amominu\.w\.aq|amominu\.w\.rel|amominu\.w\.aqrl|amomaxu\.w|amomaxu\.w\.aq|amomaxu\.w\.rel|amomaxu\.w\.aqrl)[[:alnum:]_.$]@!/
syntax match riscletDirective /\v%(^[ \t]*%(%(%(%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!)@![A-Za-z_$][[:alnum:]_.$]*|%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)|'%([^'\\\r\n]|\\[ntr\\'"0]|')')[ \t]*:[ \t]*)?)@<=\.%(global|globl|equ|set|text|data|bss|space|zero|balign|ascii|string|asciz|byte|2byte|half|4byte|word)[[:alnum:]_.$]@!/
syntax match riscletLabel /\v^[ \t]*\zs%(%(%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!)@![A-Za-z_$][[:alnum:]_.$]*|%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)|'%([^'\\\r\n]|\\[ntr\\'"0]|')')%([ \t]*:)@=/
syntax match riscletFence /\v%(^[ \t]*%(%(%(%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!)@![A-Za-z_$][[:alnum:]_.$]*|%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)|'%([^'\\\r\n]|\\[ntr\\'"0]|')')[ \t]*:[ \t]*)?fence[ \t]+%([iorw]+[ \t]*,[ \t]*)?)@<=[iorw]+[[:alnum:]_.$]@!/
syntax match riscletComment /#.*/

" Standard highlight groups let the active color scheme choose the presentation.
highlight default link riscletError Error
highlight default link riscletNumericError Error
highlight default link riscletStringError Error
highlight default link riscletStatementError Error
highlight default link riscletDirectiveError Error
highlight default link riscletCharacterError Error
highlight default link riscletEscapeError Error
highlight default link riscletSymbol Identifier
highlight default link riscletOperator Operator
highlight default link riscletPunctuation Delimiter
highlight default link riscletAddress Constant
highlight default link riscletRegister Special
highlight default link riscletNumber Number
highlight default link riscletReference Label
highlight default link riscletCharacter Character
highlight default link riscletString String
highlight default link riscletEscape SpecialChar
highlight default link riscletInstruction Statement
highlight default link riscletDirective PreProc
highlight default link riscletLabel Label
highlight default link riscletFence Constant
highlight default link riscletComment Comment

let b:current_syntax = "risclet"
