import { fileURLToPath } from "node:url";

// Guarded patches keep the pinned terminal renderer aligned at fractional zoom.
export function terminalRules() {
    return [
        { test: /@wterm\/dom\/dist\/wterm\.js$/, use: [
            fileURLToPath(new URL("wterm-viewport-loader.cjs", import.meta.url)),
            fileURLToPath(new URL("wterm-response-loader.cjs", import.meta.url)),
        ] },
        { test: /@wterm\/dom\/dist\/renderer\.js$/, use: fileURLToPath(new URL("wterm-renderer-loader.cjs", import.meta.url)) },
    ];
}
