export interface ExampleDescription {
    readonly id: string;
    readonly title: string;
    readonly editable: string;
    readonly documentation?: string;
    readonly files: ReadonlyMap<string, Uint8Array>;
}

function relativePath(value: unknown): string {
    if (typeof value !== "string" || value.includes("\\") || value.includes("\0")
        || value.split("/").some(part => !part || part === "." || part === "..")) {
        throw new Error(`Invalid example path: ${JSON.stringify(value)}`);
    }
    return value;
}

// Validate the complete bundle before any example can replace the live workspace.
function parseExample(value: unknown): ExampleDescription {
    if (typeof value !== "object" || value === null
        || !("id" in value) || !("title" in value) || typeof value.title !== "string"
        || !("editable" in value) || !("files" in value) || !Array.isArray(value.files)) {
        throw new Error("Invalid example description");
    }
    const id = relativePath(value.id);
    if (id.includes("/")) throw new Error("Example ID must be one path component");
    const files = new Map<string, Uint8Array>();
    const entries: readonly unknown[] = value.files;
    for (const file of entries) {
        if (typeof file !== "object" || file === null || !("path" in file)
            || !("size" in file) || typeof file.size !== "number" || !Number.isSafeInteger(file.size) || file.size < 0
            || !("content" in file) || typeof file.content !== "string") {
            throw new Error("Invalid bundled file");
        }
        const path = relativePath(file.path);
        if (files.has(path)) throw new Error(`Duplicate bundled path: ${path}`);
        const bytes = Uint8Array.from(atob(file.content), character => character.charCodeAt(0));
        if (bytes.length !== file.size) throw new Error(`Incorrect bundled file size: ${path}`);
        files.set(path, bytes);
    }
    for (const path of files.keys()) {
        const parts = path.split("/");
        for (let end = 1; end < parts.length; end++) {
            if (files.has(parts.slice(0, end).join("/"))) throw new Error(`File is also a directory: ${path}`);
        }
    }
    const editable = relativePath(value.editable);
    if (!files.has(editable)) throw new Error(`Missing editable file: ${editable}`);
    const documentation = "documentation" in value ? relativePath(value.documentation) : undefined;
    if (documentation !== undefined && !files.has(documentation)) throw new Error(`Missing documentation: ${documentation}`);
    return { id, title: value.title, editable, files, ...(documentation === undefined ? {} : { documentation }) };
}

export function parseExamples(value: unknown): ExampleDescription[] {
    if (typeof value !== "object" || value === null || !("version" in value) || value.version !== 1
        || !("examples" in value) || !Array.isArray(value.examples) || value.examples.length === 0) {
        throw new Error("Invalid examples bundle");
    }
    const examples = value.examples.map((example: unknown) => parseExample(example));
    if (new Set(examples.map(example => example.id)).size !== examples.length) throw new Error("Duplicate example IDs");
    return examples;
}

// One compressed response supplies every file; switching never downloads sources.
export async function loadExamples(url: string): Promise<ExampleDescription[]> {
    const response = await fetch(url);
    if (!response.ok || response.body === null) throw new Error(`Could not load examples: HTTP ${response.status}`);
    const decoded = response.body.pipeThrough(new DecompressionStream("gzip"));
    const value: unknown = await new Response(decoded).json();
    return parseExamples(value);
}
