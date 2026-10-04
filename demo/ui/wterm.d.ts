import "@wterm/dom";

// The guarded loader separates terminal query replies from keyboard/paste input.
declare module "@wterm/dom" {
    interface WTermOptions {
        onResponse?: (data: string) => void;
    }
}
