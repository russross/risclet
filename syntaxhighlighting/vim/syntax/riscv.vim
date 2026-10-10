" Highlighting follows the case-sensitive, per-line assembler tokenizer and parser.
if exists("b:current_syntax")
  finish
endif
syntax case match

" Generic tokens come first; complete literals and statement context override them.
syntax match riscvError /\v[^ \t\r]/
syntax match riscvSymbol /\v%([A-Za-z_$]|\.L)[[:alnum:]_.$]*/
syntax match riscvOperator /\v\<\<|\>\>|[+*/%\&|^~\-]/
syntax match riscvPunctuation /[,():]/
syntax match riscvAddress /\./
syntax match riscvDirectiveError /\v\.L@![[:alnum:]][[:alnum:]_.$]*/
syntax match riscvSymbol /\v\.L[[:alnum:]_.$]*/
syntax match riscvRegister /\v[[:alnum:]_.$]@<!%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!/
syntax match riscvNumber /\v%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)/
syntax match riscvNumericError /\v0%([xX][0-9a-fA-F]@!|[bB][01]@!|[oO][0-7]@!)/
" Hexadecimal f and b digits belong to the number unless whitespace precedes the suffix.
syntax match riscvReference /\v[[:alnum:]_.$]@<!%(0[xX][0-9a-fA-F]+[ \t]+|%(0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)[ \t]*)[fb][[:alnum:]_.$]@!/

" Quoted tokens stop on the physical line, and only assembler escapes are valid.
syntax match riscvCharacterError /'[^\r\n]*/
syntax match riscvCharacter /\v'%([^'\\\r\n]|\\[ntr\\'"0]|')'/
syntax match riscvStringError /"[^\r\n]*/
syntax region riscvString start=/"/ skip=/\\./ end=/"/ oneline contains=riscvEscape,riscvEscapeError
syntax match riscvEscapeError /\\./ contained
syntax match riscvEscape /\\[ntr\\'"0]/ contained

" A single non-register label can precede an opcode or directive.
syntax match riscvStatementError /\v%(^[ \t]*%(%(%(%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!)@!%([A-Za-z_$]|\.L)[[:alnum:]_.$]*|%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)|'%([^'\\\r\n]|\\[ntr\\'"0]|')')[ \t]*:[ \t]*)?)@<=%(%([A-Za-z_$]|\.L)[[:alnum:]_.$]*|\.[[:alnum:]_.$]+)/
syntax match riscvInstruction /\v%(^[ \t]*%(%(%(%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!)@!%([A-Za-z_$]|\.L)[[:alnum:]_.$]*|%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)|'%([^'\\\r\n]|\\[ntr\\'"0]|')')[ \t]*:[ \t]*)?)@<=%(add|sub|sll|slt|sltu|xor|srl|sra|or|and|mul|mulh|mulhsu|mulhu|div|divu|rem|remu|addi|slli|slti|sltiu|xori|ori|andi|srli|srai|jalr|beq|bne|blt|bge|bltu|bgeu|bgez|bnez|lui|auipc|jal|fence|fence\.tso|fence\.i|ecall|ebreak|lb|lh|lw|lbu|lhu|sb|sh|sw|li|la|call|tail|mv|ret|nop|neg|seqz|snez|sltz|sgtz|beqz|blez|bltz|bgtz|bgt|ble|bgtu|bleu|j|jr|not|c\.add|c\.mv|c\.jr|c\.jalr|c\.li|c\.lui|c\.addi|c\.addi16sp|c\.addi4spn|c\.slli|c\.lwsp|c\.swsp|c\.lw|c\.sw|c\.and|c\.or|c\.xor|c\.sub|c\.srli|c\.srai|c\.andi|c\.beqz|c\.bnez|c\.j|c\.jal|c\.nop|c\.ebreak|lr\.w|lr\.w\.aq|lr\.w\.rl|lr\.w\.aqrl|sc\.w|sc\.w\.aq|sc\.w\.rl|sc\.w\.aqrl|amoswap\.w|amoswap\.w\.aq|amoswap\.w\.rl|amoswap\.w\.aqrl|amoadd\.w|amoadd\.w\.aq|amoadd\.w\.rl|amoadd\.w\.aqrl|amoxor\.w|amoxor\.w\.aq|amoxor\.w\.rl|amoxor\.w\.aqrl|amoand\.w|amoand\.w\.aq|amoand\.w\.rl|amoand\.w\.aqrl|amoor\.w|amoor\.w\.aq|amoor\.w\.rl|amoor\.w\.aqrl|amomin\.w|amomin\.w\.aq|amomin\.w\.rl|amomin\.w\.aqrl|amomax\.w|amomax\.w\.aq|amomax\.w\.rl|amomax\.w\.aqrl|amominu\.w|amominu\.w\.aq|amominu\.w\.rl|amominu\.w\.aqrl|amomaxu\.w|amomaxu\.w\.aq|amomaxu\.w\.rl|amomaxu\.w\.aqrl)[[:alnum:]_.$]@!/
syntax match riscvDirective /\v%(^[ \t]*%(%(%(%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!)@!%([A-Za-z_$]|\.L)[[:alnum:]_.$]*|%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)|'%([^'\\\r\n]|\\[ntr\\'"0]|')')[ \t]*:[ \t]*)?)@<=\.%(global|globl|equ|set|text|data|bss|space|zero|balign|ascii|string|asciz|byte|2byte|half|4byte|word)[[:alnum:]_.$]@!/
syntax match riscvLabel /\v^[ \t]*\zs%(%(%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!)@!%([A-Za-z_$]|\.L)[[:alnum:]_.$]*|%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)|'%([^'\\\r\n]|\\[ntr\\'"0]|')')%([ \t]*:)@=/
syntax match riscvFence /\v%(^[ \t]*%(%(%(%(x%([0-9]|[12][0-9]|3[01])|zero|ra|sp|gp|tp|fp|t[0-6]|s%([0-9]|1[01])|a[0-7])[[:alnum:]_.$]@!)@!%([A-Za-z_$]|\.L)[[:alnum:]_.$]*|%(0[xX][0-9a-fA-F]+|0[bB][01]+|0[oO][0-7]+|0[0-7]*|[1-9][0-9]*)|'%([^'\\\r\n]|\\[ntr\\'"0]|')')[ \t]*:[ \t]*)?fence[ \t]+%([iorw]+[ \t]*,[ \t]*)?)@<=[iorw]+[[:alnum:]_.$]@!/
syntax match riscvComment /#.*/

" Standard highlight groups let the active color scheme choose the presentation.
highlight default link riscvError Error
highlight default link riscvNumericError Error
highlight default link riscvStringError Error
highlight default link riscvStatementError Error
highlight default link riscvDirectiveError Error
highlight default link riscvCharacterError Error
highlight default link riscvEscapeError Error
highlight default link riscvSymbol Identifier
highlight default link riscvOperator Operator
highlight default link riscvPunctuation Delimiter
highlight default link riscvAddress Constant
highlight default link riscvRegister Special
highlight default link riscvNumber Number
highlight default link riscvReference Label
highlight default link riscvCharacter Character
highlight default link riscvString String
highlight default link riscvEscape SpecialChar
highlight default link riscvInstruction Statement
highlight default link riscvDirective PreProc
highlight default link riscvLabel Label
highlight default link riscvFence Constant
highlight default link riscvComment Comment

let b:current_syntax = "riscv"
