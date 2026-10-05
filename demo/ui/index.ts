import Split from "split.js";
import { EditorSession } from "./editor-session";
import { renderInstructions } from "./instructions";
import { renderFileTree as renderWorkspaceTree } from "./workspace-view";
import { changeAffectsPath } from "./workspace";
import { VmSession } from "./vm-session";
import type { VmImage, VmTarget, VmTransition } from "./vm-session";
import type { Riscbox } from "@riscbox/runtime";
import type { P9Change } from "@riscbox/storage";
import { loadExamples } from "./examples";
import type { ExampleDescription } from "./examples";

interface ExampleState extends VmTarget { readonly description: ExampleDescription; }
declare global { interface Window { Riscbox: typeof Riscbox; } }

const switchTransition: VmTransition = {
    workspace: "snapshot", poweroff: "force", discardDiskChanges: true, boot: false,
};
const image: VmImage = {
    configUrl: new URL("riscbox.cfg", window.location.href).href,
    runtimeUrl: new URL("riscbox/riscbox.js", window.location.href).href,
    wasmUrl: new URL("riscbox/riscbox.wasm", window.location.href).href,
    memoryMiB: 256, shareName: "default",
};
let examples: ExampleState[] = [];
let currentExample: ExampleState | null = null;
let editor: EditorSession;
let vm: VmSession<ExampleState>;
let viewGeneration = 0;
let instructionsGeneration = 0;
let instructionPaths = new Set<string>();
let switchQueue = Promise.resolve();
let switching = false;
let recovering = false;
let fileSelectionGeneration = 0;

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
        selectedPath: editor.path, priority: () => 0,
        onSelect: path => {
            if (switching || recovering) return;
            const view = viewGeneration;
            const selection = ++fileSelectionGeneration;
            void editor.flush("selection").then(() => {
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
        vm.fit();
        if (!switching && !recovering) void editor.flush("interaction").then(() => vm.bootIfInactive()).catch(reportUiError);
    }
}
async function updateInstructions(): Promise<void> {
    const view = viewGeneration;
    const request = ++instructionsGeneration;
    const example = currentExample;
    const documentPath = example?.description.documentation;
    const dependencies = new Set(documentPath === undefined ? [] : [documentPath]);
    let rendered: string;
    try { rendered = documentPath === undefined ? "" : await renderInstructions(vm.filesystem, dependencies, documentPath); }
    catch (error: unknown) {
        if (view !== viewGeneration || request !== instructionsGeneration) return;
        instructionPaths = dependencies;
        throw error;
    }
    if (view !== viewGeneration || request !== instructionsGeneration || example !== currentExample) return;
    instructionPaths = dependencies;
    const button = requiredButton("instructions-tab-button");
    const content = requiredElement("instructions-tab-content");
    button.hidden = rendered === "";
    content.innerHTML = rendered;
    if (rendered === "" && content.classList.contains("active")) selectTab("vm");
}
function handleFilesystemChange(change: P9Change): void {
    if (switching || recovering || currentExample === null) return;
    try {
        editor.handleChange(change);
        if (change.kind !== "write") renderFileTree();
        if ([...instructionPaths].some(path => changeAffectsPath(change, path))) {
            void updateInstructions().catch(reportUiError);
        }
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
        await editor.flush("transition");
        if (generation !== viewGeneration) return;
        switching = true;
        updateControls();
        const wasReadOnly = editor.readOnly;
        editor.setReadOnly(true);
        let selected = false;
        try {
            const isCurrent = (): boolean => generation === viewGeneration;
            const changed = await vm.setTarget(example, switchTransition, isCurrent);
            if (changed) {
                await showExample(example, generation);
                selected = true;
            }
        } finally {
            switching = false;
            if (!selected) editor.setReadOnly(wasReadOnly);
            updateControls();
        }
        if (selected && requiredElement("vm-tab-content").classList.contains("active")) vm.bootIfInactive();
    });
    switchQueue = selection.then(() => undefined, reportUiError);
    return selection;
}

async function showExample(example: ExampleState, generation: number): Promise<void> {
    currentExample = example;
    renderMenu();
    editor.clear();
    renderFileTree();
    await updateInstructions();
    if (generation !== viewGeneration) return;
    const paths = vm.filesystem.listFiles();
    const preferred = paths.includes(example.description.editable) ? example.description.editable : paths[0];
    if (preferred !== undefined) openFile(preferred);
    if (generation !== viewGeneration) return;
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
        await vm.forceHalt();
        await vm.setTarget(target, { ...switchTransition, boot: true }, () => true);
    } finally {
        recovering = false;
        editor.setReadOnly(readOnly);
        renderMenu();
        renderFileTree();
        updateControls();
    }
}

async function initialize(): Promise<void> {
    let resized = false;
    const split = Split(["#file-tree-pane", "#editor-pane", "#info-pane"], {
        sizes: [10, 45, 45], gutterSize: 8, cursor: "grabbing",
        onDrag: () => { resized = true; vm.fit(); },
    });
    editor = new EditorSession(requiredElement("editor-pane"), {
        canEdit: () => true, onChange: updateControls, onSynced: () => {}, onError: reportUiError,
        confirmDiscard: () => window.confirm("The filesystem changed this file while you have unflushed edits. Discard your edits and use the filesystem version? Keeping your edits will replace the filesystem version on the next flush."),
    });
    vm = new VmSession(requiredElement("vm-terminal"), {
        loadRuntime: async () => window.Riscbox,
        flushEditor: () => editor.flush("interaction"), canInteract: () => !switching && !recovering,
        beforeReplace: () => { if (!recovering) { editor.clear(); currentExample = null; } },
        afterSnapshot: async () => {}, onFilesystemChange: handleFilesystemChange,
        onError: reportUiError,
        onStateChange: updateControls,
    });

    // Measure the visible grid once, including pane padding and Split's gutter.
    const terminalTab = requiredElement("vm-tab-content");
    terminalTab.classList.add("active");
    requiredElement("instructions-tab-content").classList.remove("active");
    await vm.terminal.ready;
    await document.fonts.ready;
    vm.fit();
    const surface = requiredElement("vm-terminal").querySelector<HTMLElement>(".terminal-surface");
    if (surface === null) throw new Error("Missing terminal surface");
    const cellWidth = parseFloat(getComputedStyle(surface).getPropertyValue("--term-cell-width"));
    const paneWidth = requiredElement("info-pane").getBoundingClientRect().width;
    const overhead = paneWidth - surface.clientWidth;

    // Split subtracts half a gutter from the final pane's percentage width.
    const mainWidth = requiredElement("main-content").clientWidth;
    const gridWidth = Math.ceil(80 * cellWidth) + 1;
    const percentage = Math.min(70, Math.max(45, 100 * (gridWidth + overhead + 4) / mainWidth));
    if (!resized) split.setSizes([10, 90 - percentage, percentage]);
    vm.fit();
    terminalTab.classList.remove("active");
    requiredElement("instructions-tab-content").classList.add("active");
    requiredButton("vm-boot-button").addEventListener("click", () => {
        if (vm.state === "stopping") { void recoverVm().catch(reportUiError); return; }
        void editor.flush("interaction").then(async () => {
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
    await vm.prepareImage(image);
    examples = (await loadExamples(assetUrl("examples"))).map(description => ({
        description, image, loadFiles: async () => description.files,
    }));
    renderMenu();
    if (examples.length === 0) throw new Error("No examples are configured");
    const requested = new URL(window.location.href).searchParams.get("example");
    await switchExample(examples.find(example => example.description.id === requested) ?? examples[0]);
}

document.addEventListener("DOMContentLoaded", () => {
    void initialize().catch(error => { console.error("Could not start the Risclet demo", error); reportUiError(error); });
});
