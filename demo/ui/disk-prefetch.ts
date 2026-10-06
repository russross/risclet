// Prefetch warms the browser cache without accessing the VM or its disk overlay.
export class DiskPrefetch {
    private readonly controller = new AbortController();
    private started = false;

    stop(): void { this.controller.abort(); }

    async start(configUrl: string): Promise<void> {
        if (this.started || this.controller.signal.aborted) return;
        this.started = true;
        const signal = this.controller.signal;
        try {
            const configResponse = await fetch(configUrl, { cache: "no-cache", signal });
            if (!configResponse.ok) return;
            const config: unknown = await configResponse.json();
            if (typeof config !== "object" || config === null || !("drive0" in config)) return;
            const drive = config.drive0;
            if (typeof drive !== "object" || drive === null || !("file" in drive) || typeof drive.file !== "string") return;

            // Configuration paths are relative to the config; blocks are relative to the manifest.
            const manifestUrl = new URL(drive.file, configUrl);
            const manifestResponse = await fetch(manifestUrl, { signal });
            if (!manifestResponse.ok) return;
            const manifest = await manifestResponse.text();
            const match = /^\s*\{\s*block_size:\s*(\d+),\s*n_block:\s*(\d+),?\s*\}\s*$/.exec(manifest);
            if (match === null) return;
            const count = Number(match[2]);
            if (!Number.isSafeInteger(count) || Number(match[1]) <= 0) return;

            // Each worker drains its response before taking another block, bounding active transfers.
            let next = 0;
            const worker = async (): Promise<void> => {
                while (!signal.aborted && next < count) {
                    const index = next++;
                    const url = new URL(`blk${String(index).padStart(9, "0")}.bin`, manifestUrl);
                    try {
                        const response = await fetch(url, { signal });
                        await response.arrayBuffer();
                    } catch {
                        // Missing blocks and network failures do not affect startup or other blocks.
                    }
                }
            };
            await Promise.all([worker(), worker()]);
        } catch {
            // Configuration failures and cancellation end this optional cache warmup silently.
        }
    }
}
