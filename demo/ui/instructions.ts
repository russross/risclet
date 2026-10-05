import * as commonmark from "commonmark";
import type { Filesystem, P9Change } from "@riscbox/storage";
import { changeAffectsPath } from "./workspace";

const decoder = new TextDecoder();
const markdownParser = new commonmark.Parser();
const markdownRenderer = new commonmark.HtmlRenderer({ safe: true });
type ReadmeFilesystem = Pick<Filesystem, "listFiles" | "readFile">;

function imageData(content: Uint8Array, type: string): string {
    const chunks: string[] = [];
    for (let offset = 0; offset < content.length; offset += 32768) {
        chunks.push(String.fromCharCode(...content.subarray(offset, offset + 32768)));
    }
    return `data:${type};base64,${btoa(chunks.join(""))}`;
}

function imageMimeType(path: string): string {
    switch (path.split(".").pop()?.toLowerCase()) {
        case "gif": return "image/gif";
        case "jpg":
        case "jpeg": return "image/jpeg";
        case "png": return "image/png";
        case "webp": return "image/webp";
        case "svg": return "image/svg+xml";
        default: throw new Error(`README image has an unsupported type: ${path}`);
    }
}

// Markdown HTML and unsafe URLs are filtered before filesystem images are attached.
function renderInstructions(filesystem: ReadmeFilesystem, documentPath: string,
    dependencies: Set<string>): DocumentFragment {
    const document = markdownParser.parse(decoder.decode(filesystem.readFile(documentPath)));
    const documentUrl = new URL(documentPath, "https://workspace.invalid/");
    const walker = document.walker();
    const images = new Map<string, string>();
    let event = walker.next();
    while (event !== null) {
        if (event.entering && event.node.type === "image" && event.node.destination !== null) {
            const url = new URL(event.node.destination, documentUrl);
            if (url.origin === documentUrl.origin) {
                const path = decodeURIComponent(url.pathname.replace(/^\//, ""));
                dependencies.add(path);
                const type = imageMimeType(path);
                images.set(url.href, imageData(filesystem.readFile(path), type));
                event.node.destination = url.href;
            }
        }
        event = walker.next();
    }
    const template = window.document.createElement("template");
    template.innerHTML = markdownRenderer.render(document);
    // SVG data URLs stay in image context and never gain the host page's origin.
    for (const image of template.content.querySelectorAll("img")) {
        const content = images.get(image.getAttribute("src") ?? "");
        if (content !== undefined) image.src = content;
    }
    return template.content;
}

// The pane owns documentation dependencies and contains rendering errors locally.
export class InstructionsPane {
    private documentPath: string | undefined;
    private dependencies = new Set<string>();

    constructor(private readonly host: HTMLElement, private readonly button: HTMLButtonElement) {}
    get visible(): boolean { return !this.button.hidden; }

    update(filesystem: ReadmeFilesystem, documentPath: string | undefined): void {
        this.documentPath = documentPath;
        this.dependencies = new Set(documentPath === undefined ? [] : [documentPath]);
        this.host.replaceChildren();
        this.button.hidden = documentPath === undefined || !filesystem.listFiles().includes(documentPath);
        if (documentPath === undefined || this.button.hidden) return;
        try {
            this.host.append(renderInstructions(filesystem, documentPath, this.dependencies));
        } catch (error: unknown) {
            const message = window.document.createElement("p");
            message.setAttribute("role", "status");
            message.textContent = `Could not display README: ${error instanceof Error ? error.message : String(error)}`;
            this.host.append(message);
        }
    }

    handleChange(filesystem: ReadmeFilesystem, change: P9Change): void {
        if ([...this.dependencies].some(path => changeAffectsPath(change, path))) this.update(filesystem, this.documentPath);
    }
}
