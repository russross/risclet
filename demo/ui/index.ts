import Split from "split.js";
import { EditorSession } from "./editor-session";
import { InstructionsPane } from "./instructions";
import { renderFileTree as renderWorkspaceTree } from "./workspace-view";
import { VmSession } from "./vm-session";
import type { VmImage, VmWorkspace } from "./vm-session";
import { TerminalView } from "./terminal";
import type { Riscbox } from "@riscbox/runtime";
import type { P9Change } from "@riscbox/storage";
import { loadExamples } from "./examples";
import type { ExampleDescription } from "./examples";
import { DiskPrefetch } from "./disk-prefetch";

interface ExampleState extends VmWorkspace { readonly description: ExampleDescription; }
declare global { interface Window { Riscbox: typeof Riscbox; } }

let examples: ExampleState[] = [];
let currentExample: ExampleState | null = null;
let editor: EditorSession;
let vm: VmSession;
let terminal: TerminalView;
let instructions: InstructionsPane;
let viewGeneration = 0;
let switchQueue = Promise.resolve();
let switching = false;
let recovering = false;
let fileSelectionGeneration = 0;
const diskPrefetch = new DiskPrefetch();

function assetUrl(name: string): string {
    const meta = document.querySelector<HTMLMetaElement>(`meta[name="risclet-${name}"]`);
    if (!meta?.content) throw new Error(`Missing deployed asset: ${name}`);
    return new URL(meta.content, window.location.href).href;
}

function requiredElement(id: string): HTMLElement {
    const result = document.getElementById(id);
    if (!(result instanceof HTMLElement)) throw new Error(`Missing required element: ${id}`);
    return result;
}
function requiredButton(id: string): HTMLButtonElement {
    const result = requiredElement(id);
    if (!(result instanceof HTMLButtonElement)) throw new Error(`Missing required button: ${id}`);
    return result;
}
function reportUiError(error: unknown): void {
    console.error(error);
    window.alert(error instanceof Error ? error.message : String(error));
}

// Controls reflect shared state without participating in VM lifecycle.
function updateControls(): void {
    if (vm === undefined) return;
    const boot = requiredButton("vm-boot-button");
    boot.disabled = switching || recovering || vm.target === undefined || vm.state === "loading";
    boot.textContent = vm.state === "stopping" ? "Reset VM" : vm.state === "running" ? "Reboot VM"
        : vm.state === "loading" ? "Preparing VM…" : "Boot VM";
}
function renderFileTree(): void {
    const paths = currentExample === null ? [] : vm.filesystem.listFiles();
    if (editor.path !== null && !paths.includes(editor.path) && !editor.dirty) editor.clear();
    renderWorkspaceTree(requiredElement("file-tree-pane"), paths, {
        selectedPath: editor.path,
        onSelect: path => {
            if (switching || recovering) return;
            const view = viewGeneration;
            const selection = ++fileSelectionGeneration;
            void editor.flush().then(() => {
                if (view === viewGeneration && selection === fileSelectionGeneration && !switching && !recovering) openFile(path);
            }).catch(reportUiError);
        },
    });
}
function openFile(path: string): void {
    if (currentExample === null) return;
    try { editor.open(vm.filesystem, path); }
    catch (error: unknown) { editor.clear(); reportUiError(error); }
    renderFileTree();
}

function selectTab(name: "instructions" | "vm"): void {
    const selected = name === "instructions" && !requiredButton("instructions-tab-button").hidden ? "instructions" : "vm";
    for (const button of document.querySelectorAll<HTMLButtonElement>(".tab-button")) {
        button.classList.toggle("active", button.id === `${selected}-tab-button`);
    }
    for (const content of document.querySelectorAll<HTMLElement>(".tab-content")) {
        content.classList.toggle("active", content.id === `${selected}-tab-content`);
    }
    if (selected === "vm") {
        terminal.fit();
        if (!switching && !recovering) void activateVm().catch(reportUiError);
    }
}
async function activateVm(): Promise<void> {
    diskPrefetch.stop();
    const generation = viewGeneration;
    await editor.flush();
    if (generation !== viewGeneration || switching || recovering) return;
    if (vm.state === "ready" || vm.state === "halted" || vm.state === "failed") {
        await vm.boot();
        if (generation !== viewGeneration) return;
        const target = examples.find(example => example === vm.target);
        if (target !== undefined && currentExample !== target) showExample(target);
    } else if (vm.state === "running") terminal.focus();
}
function handleFilesystemChange(change: P9Change): void {
    if (switching || recovering || currentExample === null) return;
    try {
        editor.handleChange(change);
        if (change.kind !== "write") renderFileTree();
        instructions.handleChange(vm.filesystem, change);
        if (!instructions.visible && requiredElement("instructions-tab-content").classList.contains("active")) selectTab("vm");
    } catch (error: unknown) { reportUiError(error); }
}

function renderMenu(): void {
    const menu = requiredElement("menu-items");
    menu.replaceChildren();
    const label = document.createElement("span");
    label.className = "menu-label";
    label.textContent = examples.length === 1 ? "Example:" : "Examples:";
    menu.append(label);
    for (const example of examples) {
        const button = document.createElement("button");
        button.className = "example-button";
        button.textContent = example.description.title;
        button.disabled = recovering || example === currentExample;
        button.addEventListener("click", () => { void switchExample(example).catch(reportUiError); });
        menu.append(button);
    }
}

// Example switches retain their namespaces while cold-resetting the shared VM.
function switchExample(example: ExampleState): Promise<void> {
    if (recovering) return Promise.resolve();
    const generation = ++viewGeneration;
    for (const button of document.querySelectorAll<HTMLButtonElement>(".example-button")) button.disabled = false;
    const selection = switchQueue.then(async () => {
        if (generation !== viewGeneration) return;
        await editor.flush();
        if (generation !== viewGeneration) return;
        switching = true;
        updateControls();
        const wasReadOnly = editor.readOnly;
        editor.setReadOnly(true);
        let selected = false;
        try {
            const isCurrent = (): boolean => generation === viewGeneration;
            const changed = await vm.switchWorkspace(example, isCurrent);
            if (changed) {
                showExample(example);
                selected = true;
            }
        } finally {
            switching = false;
            if (!selected) editor.setReadOnly(wasReadOnly);
            updateControls();
        }
        if (selected && requiredElement("vm-tab-content").classList.contains("active")) await activateVm();
    });
    switchQueue = selection.catch(() => undefined);
    return selection;
}

function showExample(example: ExampleState): void {
    currentExample = example;
    renderMenu();
    editor.clear();
    renderFileTree();
    instructions.update(vm.filesystem, example.description.documentation);
    const paths = vm.filesystem.listFiles();
    const preferred = paths.includes(example.description.editable) ? example.description.editable : paths[0];
    if (preferred !== undefined) openFile(preferred);
    selectTab(example.description.documentation !== undefined ? "instructions" : "vm");
    const url = new URL(window.location.href);
    url.searchParams.set("example", example.description.id);
    window.history.replaceState(null, "", url);
}

// Recovery keeps the full workspace and buffered editor text, without a flush.
async function recoverVm(): Promise<void> {
    const target = vm.target;
    if (target === undefined || switching || recovering) return;
    recovering = true;
    const readOnly = editor.readOnly;
    editor.setReadOnly(true);
    renderMenu();
    updateControls();
    try {
        await vm.recover();
    } finally {
        recovering = false;
        editor.setReadOnly(readOnly);
        renderMenu();
        renderFileTree();
        updateControls();
    }
}

async function initialize(): Promise<void> {
    const pageLoaded = document.readyState === "complete" ? Promise.resolve()
        : new Promise<void>(resolve => window.addEventListener("load", () => resolve(), { once: true }));
    Split(["#file-tree-pane", "#editor-pane", "#info-pane"], {
        sizes: [10, 45, 45], minSize: 0, gutterSize: 8, cursor: "grabbing",
    });
    editor = new EditorSession(requiredElement("editor-pane"), {
        onError: reportUiError,
        confirmDiscard: () => window.confirm("The filesystem changed this file while you have unflushed edits. Discard your edits and use the filesystem version? Keeping your edits will replace the filesystem version on the next flush."),
    });
    const sendInput = (bytes: Uint8Array): void => {
        if (!switching && !recovering && vm.state === "running") void vm.sendInput(bytes, editor.flush()).catch(reportUiError);
    };
    terminal = new TerminalView(requiredElement("vm-terminal"), {
        onData: text => sendInput(new TextEncoder().encode(text)), onBinary: sendInput,
        onResponse: text => vm.sendResponse(new TextEncoder().encode(text)),
        onResize: (columns, rows) => vm?.resize(columns, rows),
    });
    instructions = new InstructionsPane(requiredElement("instructions-tab-content"), requiredButton("instructions-tab-button"));
    const image: VmImage = { configUrl: assetUrl("config"), wasmUrl: assetUrl("wasm"), memoryMiB: 256, shareName: "default" };
    vm = new VmSession(window.Riscbox, image, terminal, {
        onFilesystemChange: handleFilesystemChange,
        onError: reportUiError,
        onStateChange: updateControls,
    });

    requiredButton("vm-boot-button").addEventListener("click", () => {
        diskPrefetch.stop();
        if (vm.state === "stopping") { void recoverVm().catch(reportUiError); return; }
        void editor.flush().then(async () => {
            const running = vm.state === "running";
            selectTab("vm");
            if (running) await vm.reboot();
        }).catch(reportUiError);
    });
    requiredButton("vm-boot-button").addEventListener("pointerdown", event => {
        if (vm.state === "stopping") event.preventDefault();
    });
    requiredButton("instructions-tab-button").addEventListener("click", () => selectTab("instructions"));
    requiredButton("vm-tab-button").addEventListener("click", () => selectTab("vm"));
    examples = (await loadExamples(assetUrl("examples"))).map(description => ({
        description, files: description.files,
    }));
    renderMenu();
    const requested = new URL(window.location.href).searchParams.get("example");
    await switchExample(examples.find(example => example.description.id === requested) ?? examples[0]);
    // Main assets, the prepared VM, and the populated editor take precedence over disk warmup.
    await Promise.all([pageLoaded, document.fonts.ready]);
    void diskPrefetch.start(image.configUrl);
}

document.addEventListener("DOMContentLoaded", () => {
    void initialize().catch(reportUiError);
});
