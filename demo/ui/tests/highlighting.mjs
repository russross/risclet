import { StringStream } from "@codemirror/language";
import { riscletStreamParser } from "../risclet.ts";

// Exercise directive recognition alongside actual strings and fill operands.
// Similar-looking unknown names must remain invalid rather than match prefixes.
export async function run() {
    for (const [line, expected] of [
        ['.word 0x1234', ['meta', 'number']],
        ['.half -1', ['meta', 'operator', 'number']],
        ['.ascii "text"', ['meta', 'string']],
        ['.string "text", ""', ['meta', 'string', 'punctuation', 'string']],
        ['.asciz "text"', ['meta', 'string']],
        ['.zero 4', ['meta', 'number']],
        ['.space 4, 255', ['meta', 'number', 'punctuation', 'number']],
        ['.wordx 4', ['invalid', 'number']],
        ['.asciix "text"', ['invalid', 'string']],
        ['.L1: nop', ['labelName', 'punctuation', 'keyword']],
        ['.Lloop: j .Lloop', ['labelName', 'punctuation', 'keyword', 'variableName']],
        ['.Ldata: .word .L1', ['labelName', 'punctuation', 'meta', 'variableName']],
        ['.equ .Lsize, . - .L1', ['meta', 'variableName', 'punctuation', 'atom', 'operator', 'variableName']],
        ['.global .Lexport', ['meta', 'variableName']],
        ['.l1', ['invalid']],
    ]) {
        const stream = new StringStream(line, 4, 2);
        const state = riscletStreamParser.startState(2);
        const actual = [];
        while (!stream.eol()) {
            stream.start = stream.pos;
            const style = riscletStreamParser.token(stream, state);
            if (style !== null) actual.push(style);
        }
        if (JSON.stringify(actual) !== JSON.stringify(expected)) {
            throw new Error(`${line}: expected ${expected}, got ${actual}`);
        }
    }
}
