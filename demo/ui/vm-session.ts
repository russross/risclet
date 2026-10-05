import type { Riscbox, RiscboxOptions } from "@riscbox/runtime";
import type { Filesystem, P9Change, BlockDisk } from "@riscbox/storage";
import type { TerminalView } from "./terminal";
import { TerminalInputQueue } from "./terminal_input";
import { populateWorkspace, restoreWorkspace, snapshotWorkspace } from "./workspace";
import type { WorkspaceSnapshot } from "./workspace";

export interface VmImage {
    readonly configUrl: string;
    readonly wasmUrl: string;
    readonly memoryMiB: number;
    readonly shareName: string;
}
export interface VmWorkspace {
    readonly files: ReadonlyMap<string, Uint8Array>;
    snapshot?: WorkspaceSnapshot;
}
export type VmState = "ready" | "loading" | "stopping" | "running" | "halted" | "failed";
export interface VmSessionOptions {
    onFilesystemChange(change: P9Change): void;
    onStateChange(state: VmState): void;
    onError(error: unknown): void;
}

// One prepared machine serves all examples; each workspace retains its own snapshot.
export class VmSession {
    private machine: Riscbox | undefined;
    private share: Filesystem | undefined;
    private disks: readonly BlockDisk[] = [];
    private selected: VmWorkspace | undefined;
    private loaded = false;
    private inputGeneration = 0;
    private runtimeGeneration = 0;
    private currentState: VmState = "ready";
    private unsubscribe: (() => void) | undefined;
    private readonly input = new TerminalInputQueue(bytes =>
        this.state === "running" ? this.machine?.consoleInput(bytes) ?? 0 : 0);

    constructor(private readonly runtimeConstructor: typeof Riscbox, private readonly image: VmImage,
        private readonly terminal: TerminalView, private readonly options: VmSessionOptions) {}

    get state(): VmState { return this.currentState; }
    get target(): VmWorkspace | undefined { return this.selected; }
    get filesystem(): Filesystem {
        if (this.share === undefined) throw new Error("VM filesystem is unavailable");
        return this.share;
    }
    private setState(state: VmState): void { this.currentState = state; this.options.onStateChange(state); }
    private clearInput(): void { this.inputGeneration += 1; this.input.clear(); }
    private clearTerminal(): void { this.clearInput(); this.terminal.clear(); }

    // Capture retirement before waiting for the caller's prerequisite work.
    async sendInput(bytes: Uint8Array, ready: Promise<void>): Promise<void> {
        if (this.state !== "running") return;
        const generation = this.inputGeneration;
        const copied = bytes.slice();
        await ready;
        if (generation === this.inputGeneration && this.state === "running") this.input.enqueue(copied);
    }
    sendResponse(bytes: Uint8Array): void {
        if (this.state === "running") this.input.enqueue(bytes, true);
    }
    resize(columns: number, rows: number): void {
        if (this.machine?.started) this.machine.consoleResize(columns, rows);
    }

    // Preparation can be retried after a failed request without retaining stale callbacks.
    async prepare(): Promise<void> {
        if (this.machine !== undefined) return;
        await this.terminal.ready;
        const generation = ++this.runtimeGeneration;
        const active = (): boolean => generation === this.runtimeGeneration;
        const callbacks: RiscboxOptions = {
            consoleWrite: text => { if (active()) this.terminal.write(text); },
            consoleReset: () => { if (active()) this.clearTerminal(); },
            onVmStarted: () => { if (active() && this.state !== "running") this.markRunning(); },
            onVmReset: () => { if (active()) this.markRunning(); },
            onVmHalted: () => { if (active()) { this.clearInput(); this.setState("halted"); } },
            onError: error => { if (active()) { this.fail(error); this.options.onError(error); } },
        };
        try {
            const response = await fetch(this.image.wasmUrl, { cache: "no-cache" });
            if (!response.ok) throw new Error(`WASM request failed with status ${response.status}`);
            const runtime = await this.runtimeConstructor.instantiate(await response.arrayBuffer(), callbacks);
            this.machine = runtime;
            const config = await this.runtimeConstructor.loadResolvedConfig(this.image.configUrl);
            const drives = [config.drive0, config.drive1, config.drive2, config.drive3]
                .filter(drive => drive !== undefined);
            if (drives.some(drive => !("file" in drive))) throw new Error("VM recovery requires HTTP-backed disks");
            await runtime.prepareResolved(config, this.image.memoryMiB);
            this.share = runtime.filesystem(this.image.shareName);
            this.disks = drives.map((_, index) => runtime.block(index));
            this.unsubscribe = this.share.subscribe(change => { if (active()) this.options.onFilesystemChange(change); });
        } catch (error: unknown) {
            await this.destroyMachine();
            throw error;
        }
    }

    // Callers serialize switches; cancellation is checked before replacing a namespace.
    async switchWorkspace(target: VmWorkspace, isCurrent: () => boolean): Promise<boolean> {
        const previous = this.selected;
        this.clearInput();
        try {
            await this.forceHalt();
            if (!isCurrent()) return false;
            if (previous !== undefined && this.loaded) previous.snapshot = snapshotWorkspace(this.filesystem);
            this.selected = target;
            this.loaded = false;
            this.setState("loading");
            await this.prepare();
            if (!isCurrent()) return false;
            const runtime = this.machine;
            if (runtime === undefined) throw new Error("VM is unavailable");
            await runtime.coldReset();
            if (!isCurrent()) return false;
            for (const disk of this.disks) disk.discardChanges();
            if (target.snapshot !== undefined) restoreWorkspace(this.filesystem, target.snapshot);
            else populateWorkspace(this.filesystem, target.files);
            this.loaded = true;
            this.setState("ready");
            return true;
        } catch (error: unknown) {
            if (isCurrent()) this.fail(error);
            throw error;
        }
    }

    // Recovery keeps the namespace and leaves buffered editor text to its owner.
    async recover(): Promise<void> {
        const target = this.selected;
        if (target === undefined) throw new Error("VM has no workspace to recover");
        await this.switchWorkspace(target, () => true);
        await this.boot();
    }
    private async forceHalt(): Promise<void> {
        this.clearInput();
        if (this.machine?.started) await this.machine.halt();
    }
    async boot(): Promise<void> {
        if (this.state === "running" || this.state === "loading") return;
        if (!this.loaded) {
            const target = this.selected;
            if (target === undefined) throw new Error("VM has no workspace to boot");
            await this.switchWorkspace(target, () => true);
        }
        this.setState("loading");
        try {
            this.terminal.fit();
            if (this.machine === undefined) throw new Error("VM is unavailable");
            await this.machine.boot();
        } catch (error: unknown) { this.fail(error); throw error; }
    }

    // Acceptance only changes the button state; reset callbacks mark the new boot.
    async reboot(): Promise<void> {
        if (this.machine === undefined) throw new Error("VM is unavailable");
        this.clearInput();
        if (!this.machine.started) { await this.boot(); return; }
        const generation = this.inputGeneration;
        this.setState("stopping");
        try { await this.machine.requestReboot(); }
        catch (error: unknown) {
            if (generation !== this.inputGeneration) return;
            this.fail(error);
            throw error;
        }
    }
    private markRunning(): void {
        this.setState("running");
        this.terminal.fit();
        this.resize(this.terminal.cols, this.terminal.rows);
        this.terminal.focus();
    }
    private fail(error: unknown): void {
        this.clearInput();
        this.setState("failed");
        this.terminal.writeln(`\r\n${error instanceof Error ? error.message : String(error)}`);
    }

    // Invalidating callbacks precedes destruction, including partial preparation failures.
    private async destroyMachine(): Promise<void> {
        this.clearInput();
        this.unsubscribe?.();
        this.unsubscribe = undefined;
        this.runtimeGeneration += 1;
        const runtime = this.machine;
        this.machine = undefined;
        this.share = undefined;
        this.disks = [];
        if (runtime !== undefined) { if (runtime.started) await runtime.halt(); await runtime.destroy(); }
    }
    async destroy(): Promise<void> { await this.destroyMachine(); }
}
