import { EditorSession } from "../editor-session.ts";
import { VmSession } from "../vm-session.ts";
import { TerminalView } from "../terminal.ts";
import { renderFileTree } from "../workspace-view.ts";
import { language } from "@codemirror/language";
import { riscletLanguage } from "../risclet.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
let editorCellWidth = 0;
function check(condition, message) { if (!condition) throw new Error(message); }
async function tick() { await new Promise(resolve => setTimeout(resolve, 0)); }
async function wait(condition, message) {
    for (let i = 0; i < 100; i++) { if (condition()) return; await tick(); }
    throw new Error(message);
}

// This boundary model controls halt completion and injected failures explicitly.
function filesystem(started = () => false) {
    const files = new Map();
    const subscribers = new Set();
    return {
        files, failWrite: false,
        readFile(path) { if (!files.has(path)) throw new Error("Missing file"); return files.get(path).slice(); },
        writeFile(path, bytes, origin = 0n) {
            if (this.failWrite) throw new Error("write failed");
            files.set(path, typeof bytes === "string" ? encoder.encode(bytes) : bytes.slice());
            this.notify({ kind: "write", path, aliases: [], source: "host", origin });
        },
        notify(change) { for (const callback of subscribers) callback(change); },
        subscribe(callback) { subscribers.add(callback); return () => subscribers.delete(callback); },
        listFiles() { return [...files.keys()]; },
        clear() { check(!started(), "namespace clear requires halt"); files.clear(); },
        stat(path) {
            return { kind: path === "" ? "directory" : "file", inode: BigInt(path === "" ? 1 : [...files.keys()].indexOf(path) + 2),
                mode: 0o644, uid: 0, gid: 0, atime: { seconds: 0n, nanoseconds: 0 }, mtime: { seconds: 0n, nanoseconds: 0 } };
        },
        listDirectory() { return [...files.keys()].map(name => ({ name })); },
        setAttributes() {},
    };
}

async function editorTests() {
    const host = document.createElement("div");
    document.body.append(host);
    const fs = filesystem();
    fs.writeFile("a.s", "original\n");
    let prompts = 0;
    let discard = false;
    const errors = [];
    const session = new EditorSession(host, { onError: error => errors.push(error),
        confirmDiscard: () => { prompts++; return discard; } });
    check(getComputedStyle(session.view.dom).fontSize === "20px", "editor ignores the user's startup font size");
    check(getComputedStyle(session.view.scrollDOM).fontFamily.includes("Latin Modern Mono"), "editor and terminal use different fonts");
    const font = getComputedStyle(session.view.scrollDOM);
    const context = document.createElement("canvas").getContext("2d");
    context.font = `${font.fontSize} ${font.fontFamily}`;
    editorCellWidth = context.measureText("0123456789").width / 10;
    fs.subscribe(change => session.handleChange(change));
    session.open(fs, "a.s");
    check(session.view.state.facet(language) === riscletLanguage, "assembly files use the teaching dialect by default");
    const edit = text => session.view.dispatch({ changes: { from: 0, to: session.view.state.doc.length, insert: text } });
    edit("buffered");
    check(decoder.decode(fs.readFile("a.s")) === "original\n", "edits remain buffered");
    fs.failWrite = true;
    await session.flush().then(() => { throw new Error("write should fail"); }, () => {});
    check(session.dirty && session.view.state.doc.toString() === "buffered", "failed write preserves dirty text");
    fs.failWrite = false;
    await session.flush();
    check(!session.dirty && decoder.decode(fs.readFile("a.s")) === "buffered\n", "retry acknowledges only a successful write");

    edit("keep");
    fs.writeFile("a.s", "external\n");
    fs.writeFile("a.s", "external again\n");
    check(prompts === 1 && session.dirty, "retained conflict prompts once");
    await session.flush();
    edit("discard");
    discard = true;
    fs.writeFile("a.s", "accepted\n");
    check(!session.dirty && session.view.state.doc.toString() === "accepted", "accepted conflict replaces dirty buffer");
    edit("renamed buffer");
    fs.files.set("renamed.s", fs.files.get("a.s"));
    fs.files.delete("a.s");
    fs.notify({ kind: "rename", oldPath: "a.s", path: "renamed.s", aliases: [], source: "guest", origin: 0n });
    await session.flush();
    check(session.path === "renamed.s" && decoder.decode(fs.readFile("renamed.s")) === "renamed buffer\n", "dirty rename follows the new path");

    fs.writeFile("binary", Uint8Array.of(0));
    session.open(fs, "binary");
    check(session.readOnly, "binary files remain read-only");
    session.open(fs, "renamed.s");
    edit("blurred");
    session.view.contentDOM.dispatchEvent(new FocusEvent("blur"));
    await wait(() => !session.dirty, "blur did not flush");
    check(decoder.decode(fs.readFile("renamed.s")) === "blurred\n", "blur writes the buffered text");

    // Recovery can restore access before a blur's queued write gets its turn.
    edit("recovery buffer");
    session.setReadOnly(true);
    const recoveryBlur = session.flush();
    session.setReadOnly(false);
    await recoveryBlur;
    check(session.dirty && decoder.decode(fs.readFile("renamed.s")) === "blurred\n",
        "read-only blur cannot flush after recovery restores editability");
    await session.flush();

    const tree = document.createElement("div");
    renderFileTree(tree, ["z.s", "nested/a.s"], { selectedPath: "nested/a.s", onSelect: () => {} });
    check(tree.querySelector("li.file").dataset.path === "nested/a.s", "directories precede files");
    check(tree.querySelector("button") !== null, "file selection has native keyboard controls");
    session.destroy();
    host.remove();
    check(errors.length === 0, "unexpected editor error");
}

async function vmTests() {
    const host = document.createElement("div");
    host.style.cssText = "width:600px;height:200px";
    document.body.append(host);
    const machines = [];
    const errors = [];
    let failConfig = true;
    let holdReset;
    let releaseReset;
    const constructor = {
        async loadResolvedConfig() {
            if (failConfig) { failConfig = false; throw new Error("configuration unavailable"); }
            return { version: 1, machine: "riscv64", memory_size: 64, drive0: { file: "https://image.invalid/disk.json" } };
        },
        async instantiate(_, callbacks) {
            const machine = { started: false, boots: 0, halted: 0, destroyed: false, discards: 0, resizes: 0, input: [], callbacks,
                async prepareResolved() {}, filesystem() { return this.fs; },
                block() { return { discardChanges: () => { this.discards++; } }; },
                async boot() { this.started = true; this.boots++; callbacks.consoleReset(); callbacks.onVmReset(); callbacks.onVmStarted(); },
                async halt() { this.started = false; this.halted++; callbacks.onVmHalted("forced"); },
                async requestReboot() {},
                async coldReset() { if (holdReset) await new Promise(resolve => { releaseReset = resolve; }); callbacks.consoleReset(); },
                async destroy() { this.destroyed = true; },
                consoleResize() { check(this.started, "console resize requires a running VM"); this.resizes++; },
                consoleInput(bytes) { this.input.push(...bytes); return bytes.length; },
            };
            machine.fs = filesystem(() => machine.started);
            machines.push(machine);
            return machine;
        },
    };
    const terminal = new TerminalView(host);
    const image = { configUrl: "https://image.invalid/a.cfg", wasmUrl: "data:application/wasm;base64,AA==",
        memoryMiB: 64, shareName: "default" };
    const target = name => ({ files: new Map([["file", encoder.encode(name)]]) });
    const first = target("first");
    const second = target("second");
    const session = new VmSession(constructor, image, terminal, {
        onFilesystemChange: () => {}, onStateChange: () => {}, onError: error => errors.push(error),
    });

    // Failed preparation releases its handles and remains retryable through Boot VM.
    await session.switchWorkspace(first, () => true).then(() => { throw new Error("configuration failure must reject"); }, () => {});
    check(session.state === "failed" && machines[0].destroyed, "failed preparation did not release the machine");
    await session.boot();
    check(session.state === "running" && machines.length === 2, "boot did not retry preparation");
    const machine = machines[1];
    check(machine.resizes === 1, "host boot processed both reset and started callbacks");
    const screen = host.querySelector(".xterm-screen");
    const cellHeight = screen.getBoundingClientRect().height / terminal.rows;
    check(Math.abs(screen.getBoundingClientRect().width / terminal.cols - editorCellWidth) < 1 / devicePixelRatio,
        "terminal character width differs from the editor's startup font");
    document.documentElement.style.fontSize = "24px";
    terminal.fit();
    check(Math.abs(screen.getBoundingClientRect().height / terminal.rows - cellHeight) < 1 / devicePixelRatio,
        "resize resampled the root font size after startup");
    machines[0].callbacks.onVmStarted();
    machines[0].callbacks.consoleWrite("retired callback");
    check(await terminal.readText() === "", "retired preparation forwarded old output");

    // A delayed editor flush cannot inject input into a replacement workspace.
    let releaseFlush;
    const pendingInput = session.sendInput(encoder.encode("obsolete"), new Promise(resolve => { releaseFlush = resolve; }));
    machine.fs.writeFile("file", "guest edits");
    await session.switchWorkspace(second, () => true);
    releaseFlush();
    await pendingInput;
    check(machine.input.length === 0, "retired editor flush injected input into a new workspace");
    check(!machine.started && session.state === "ready", "switch booted a README-first example");
    check(machine.discards === 2 && decoder.decode(first.snapshot.entries.find(entry => entry.path === "file").bytes) === "guest edits",
        "switch did not snapshot guest edits or discard disk changes");
    await session.switchWorkspace(first, () => true);
    check(decoder.decode(session.filesystem.readFile("file")) === "guest edits", "switch did not restore the retained workspace");
    await session.boot();

    // A newer selection arriving during cold reset prevents stale namespace replacement.
    let current = true;
    holdReset = true;
    const obsolete = session.switchWorkspace(second, () => current);
    await wait(() => releaseReset !== undefined, "cold reset did not begin");
    current = false;
    releaseReset();
    holdReset = false;
    check(await obsolete === false, "superseded switch completed");
    await session.switchWorkspace(first, () => true);
    check(decoder.decode(session.filesystem.readFile("file")) === "guest edits", "superseded switch overwrote a snapshot");
    await session.boot();

    // Reboot starts clearing at the runtime boundary, after visible shutdown output.
    machine.callbacks.consoleWrite("old boot history\r\n");
    await terminal.readText();
    await session.reboot();
    check(session.state === "stopping", "reboot does not wait visibly for its callback");
    check((await terminal.readText()).includes("old boot history"), "shutdown output disappeared before reboot began");
    await session.sendInput(encoder.encode("blocked"), Promise.resolve());
    check(machine.input.length === 0, "reboot wait accepted terminal input");
    machine.callbacks.consoleWrite("queued shutdown tail".repeat(2000));
    machine.callbacks.consoleReset();
    machine.callbacks.onVmReset();
    machine.callbacks.consoleWrite("new boot starts at row one");
    check(await terminal.readText() === "new boot starts at row one", "reboot replayed old output or lost fresh boot output");
    check(session.state === "running", "restart notification did not release the reboot wait");

    // A guest-initiated reboot also refreshes geometry while the app is already running.
    const resizes = machine.resizes;
    machine.callbacks.consoleReset();
    machine.callbacks.onVmReset();
    check(machine.resizes === resizes + 1, "guest reboot did not reannounce terminal geometry");

    machine.fs.writeFile("extra", "keep me");
    machine.callbacks.consoleWrite("obsolete recovery tail".repeat(2000));
    await session.recover();
    machine.callbacks.consoleWrite("recovered boot");
    check(await terminal.readText() === "recovered boot", "recovery replayed output from the previous boot");
    check(decoder.decode(session.filesystem.readFile("extra")) === "keep me", "recovery lost guest files");
    await session.destroy();
    machine.callbacks.onVmStarted();
    terminal.destroy();
    host.remove();
    check(errors.length === 0, "unexpected asynchronous VM error");
}

export async function run() { await editorTests(); await vmTests(); }
