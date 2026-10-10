Demo semantics
==============

This document records the demo's observable workflows, UX, storage lifetimes, and ordering requirements. Its purpose is to preserve a browser environment in which a student can read an exercise, edit assembly, run it, and use the full terminal debugger without installing tools. Internal libraries and message wording may change; workspace isolation, edit synchronization, terminal behavior, and recovery guarantees may not silently change with them.

[README.md](README.md) describes building and running the demo. The [core semantics](../SEMANTICS.md) govern the Risclet executable inside the guest; the demo supplies a Linux VM, shared workspace, editor, instructions, and terminal around it. In particular, the terminal must preserve the core debugger's colors, box lines, alternate screen, key controls, and resize behavior. A plain output log with an input box is not a functional substitute.


Student workflow and page interaction
-------------------------------------

The page fills the browser viewport with an example-selection bar and three horizontally arranged panes: file tree, editor, and an information pane with README/VM tabs. Initial widths are 10%/45%/45%, accounting for draggable gutters. Resizing the browser retains the assigned proportions; switching tabs does not replace them with new defaults. Pane overflow scrolls within the pane rather than making the entire page grow. The file tree may become narrow without forcing the editor or terminal offscreen.

The intended first interaction is to select an example, read its instructions, and inspect its editable file. Preparing the runtime and showing the workspace do not require booting Linux. The student chooses the VM tab when ready to execute commands. Examples without documentation enter the VM directly. Once booted, returning to README does not stop or reset the guest; returning to VM fits and focuses the existing console rather than starting another machine.

The tree shows the current workspace, including guest-created files. Group directories before files at each level and sort names within each group. Indentation expresses hierarchy; directory rows are descriptive, while file rows are keyboard-accessible buttons with persistent selection highlighting. Long paths can scroll horizontally instead of being irretrievably truncated. The current tree is expanded rather than a directory-collapse interface. Empty directories can survive snapshots even though the file-path tree does not display every namespace object independently.

The editor provides normal text editing, selection, undo, and language highlighting. Assembly highlighting follows the same supported language as the core; C/header, Markdown, Python, and Makefile text receive their corresponding supported modes. Tab inserts spaces through the next four-column boundary, including each selected range, instead of literal tab bytes. Binary notices cannot be edited. Editor and terminal share a monospace family and the browser's initial preferred text size. Browser zoom remains under the student's control.

There is no separate Save/Run synchronization step. A command typed into the terminal must observe the editor text already visible to the student because its input waits for the edit flush. Selecting a new file or example similarly commits the outgoing buffer first. A failed flush reports the error and retains the unsaved buffer; it must not allow a new selection to silently lose the text. The workspace remains session-local; page refresh starts again from packaged examples.

The selected example is reflected in the URL for a direct link to that example, without navigation or a browser-history entry for every selection. Selecting the current completed example is disabled. During an ordinary switch another example can supersede the request; during forced recovery example controls are disabled. The active tab and file remain visually identifiable, and native buttons retain keyboard focus behavior.


VM control states
-----------------

The single VM control expresses the next available operation, with a stable width so changing its label does not shift the example menu. Wording may vary, but preparing, bootable, running, and awaiting-reboot states must be distinguishable.

| Situation                                   | Control and action                                                                          |
|---------------------------------------------|---------------------------------------------------------------------------------------------|
| No target, workspace switching, or recovery | Disabled until the operation establishes a usable selection                                 |
| Preparing or booting                        | Disabled and visibly preparing; duplicate boot requests do not create another runtime       |
| Ready, halted, or failed with a target      | Boot/retry is available; a recoverable preparation failure can retry without refreshing     |
| Running                                     | Request an orderly reboot after flushing edits                                              |
| Reboot requested, callback not yet received | Offer forced reset; do not pretend the request's acceptance means the new guest has started |

A transient initial WASM/preparation failure must report the failure and leave a usable Boot retry. A successful retry reconstructs the complete selected workspace, tree, editor, and instructions, then runs the guest. It must not leave a booted machine with empty UI widgets or retain callbacks from the failed preparation.


Storage and ownership
---------------------

`ui/index.ts` coordinates user actions. One `VmSession` owns a prepared Riscbox machine, its root-disk handle, shared filesystem subscription, and terminal-input queue. `EditorSession` owns its text buffer, dirty revisions, queued writes, and conflict decisions. `InstructionsPane` owns rendered documentation and image dependencies. Filesystem events reach those views through the application.

| State                         | Owner/location                       | Lifetime                                                   |
|-------------------------------|--------------------------------------|------------------------------------------------------------|
| Bundled example bytes         | Validated example descriptions       | Immutable source data for the page session                 |
| Selected workspace            | Riscbox in-memory 9p namespace       | Survives reboot; copied before a workspace switch or reset |
| Inactive workspace            | Per-example `WorkspaceSnapshot`      | Retained in memory until page refresh                      |
| Unflushed editor text         | CodeMirror and editor revision state | Retained until flushed, explicitly discarded, or refreshed |
| Guest root filesystem changes | Runtime block-disk overlay           | Survive orderly reboot; discarded by cold recovery/switch  |

The guest mounts the shared namespace at `/home/risclet` using 9p tag `shared`, server `default`, and `cache=none`. Guest processes and host editor operations see the same files. Workspace contents include generated files and metadata, not just original example sources. There is no workspace persistence across page refreshes.


Build inputs to served page
---------------------------

1. `Makefile` selects the published Riscbox version from `version`, the published Risclet version from the root Cargo manifest or `RISCLET_VERSION`, and a pinned Alpine minirootfs. Download scripts verify digests before replacing cached files. Public runtime declarations and runtime bytes are extracted from the same Riscbox release.
2. `build-image.sh` combines the minirootfs, selected Risclet binary, and guest configuration into a 16 MiB ext4 image under fakeroot. Building the disk does not boot a VM or install guest packages. The release's splitter produces HTTP disk blocks separately from UI compilation and example packing.
3. `scripts/bundle.py` validates the example manifest and packages every declared file, including binary bytes, into a compressed JSON bundle. UI compilation imports the shared CodeMirror grammar from `../syntaxhighlighting/codemirror/riscv.ts`; it does not maintain a separate assembly grammar.
4. `scripts/site.py` creates the runtime configuration and publishes the entry page. Configuration paths resolve relative to the configuration URL. The page references content-named application, style, configuration, and example assets; runtime URLs include the selected release version. An existing immutable asset URL must not acquire incompatible bytes.

Input inventories track contents and deletions separately for UI, guest files, and examples; version stamps track selected release changes. UI edits do not require disk reconstruction, and example edits do not require UI compilation. Local incremental publication retains older immutable assets so already-loaded pages can still fetch their dependencies.

`make -C demo mostlyclean` removes browser-test intermediates while retaining the deployed site. `make -C demo clean` also removes generated downloads, images, and deployment assets. Neither target is a workspace-persistence mechanism; browser workspaces live in the page session.


Page startup to first workspace
-------------------------------

Initialization creates the editor, terminal, instructions pane, and VM session, then fetches the one gzip example bundle. Browser validation checks bundle version, nonempty examples, unique IDs and paths, decoded sizes, safe relative paths, file/directory collisions, and the presence of selected editable/documentation files. Validation completes before any example can replace the live namespace. Switching examples never fetches individual source files.

The `example` query parameter selects a matching example; an absent or unknown ID selects the first example. Initial selection follows the same serialized switching operation as later selections. Preparing Riscbox waits for terminal readiness and obtains a halted machine with 256 MiB RAM, one HTTP-backed root disk, and the configured shared filesystem. Partial preparation failure releases acquired resources; retired runtime callbacks are ignored by generation checks.

After loading the namespace, the app renders its tree, opens the preferred editable file or the first available file, updates instructions, and replaces the URL's example parameter without navigation. Examples with configured documentation select the instructions tab; others select the VM tab. Selecting the VM tab flushes edits and boots when the session is ready, halted, or failed. A prepared machine need not yet be running.

After the initial workspace is ready, page loading is complete, and fonts are ready, `DiskPrefetch` performs optional cache warming. It fetches the configuration and disk manifest and drains block responses with two concurrent workers. It does not access VM state or modify disk overlays. A boot request permanently stops prefetch for that page session, aborting active transfers and abandoning remaining blocks. Prefetch failures do not fail application startup.


Editor buffer to shared files
-----------------------------

Opening a file reads the shared filesystem only after the previous dirty buffer has been flushed or explicitly discarded. Files containing NUL bytes open as read-only binary notices. Text decoding hides one final newline; flushing a nonempty buffer appends one newline, while an empty buffer writes zero bytes. This is editor serialization, not byte-preserving binary editing.

User edits advance a revision counter and restart a 30-second inactivity timer. Blur, file selection, example selection, VM activation, and user terminal input also request a flush. Flush operations serialize through a promise queue. A successful filesystem write acknowledges the revision and tags the write with `EDITOR_ORIGIN`; a failed write retains dirty text and schedules another opportunity to flush. Selection does not proceed through a failed prerequisite flush.

Filesystem changes from the guest or other host operations update the tree, editor, and instructions. Own editor writes do not create conflicts. Aliases, ancestor paths, reset/rescan events, and rename source paths participate in determining which views are affected. Renaming the selected path or an ancestor moves the selected buffer with it.

External content changes to a dirty buffer require a discard decision. Accepting reloads the filesystem version or clears a removed selection. Declining retains the buffer and suppresses repeated prompts for that conflict; its next flush replaces the filesystem version. Renames and metadata events preserve dirty text without that content-discard decision. Read-only recovery state prevents a blur-triggered queued flush from later writing after recovery.


Example switch transaction
--------------------------

`switchExample` serializes requests through `switchQueue` and increments a view generation as soon as a selection is requested. The latest request wins. File selections and VM activation also check generations after awaiting work, so delayed completion cannot select a stale view.

1. Verify that the request is current, flush the outgoing editor, check currency again, mark switching active, and make the editor read-only.
2. Retire queued/delayed terminal input and force-halt the machine. Once halted, snapshot the outgoing namespace if it was fully loaded. Cancellation is checked before replacing the namespace.
3. Select the target, mark it unloaded, prepare the runtime if necessary, and cold-reset it. Check currency after asynchronous preparation/reset, then discard root-disk overlays.
4. Restore the target's saved snapshot, or populate its original bundled files on first selection. Only after population succeeds is the target marked loaded and the VM state set to `ready`.
5. Publish the selected example to the views, restore editing, and boot if its selected tab requires the VM. Failed or superseded work must not publish a stale selection; queue rejection is contained so later selections can proceed.

Snapshots traverse the complete namespace while the guest is halted. They preserve directories, file bytes, symlink targets, hard-link relationships, and attributes. Restoration clears the namespace, recreates parents and original link targets before dependents, then restores attributes in reverse order so creation does not overwrite saved directory timestamps. First-time population creates parent directories before writing bundled bytes.

Switching is ordered and cancellable, but does not promise rollback of every partially completed step. `loaded` distinguishes a completed namespace from an interrupted load so retries do not snapshot incomplete state as a valid example.


Boot, reboot, and recovery
--------------------------

VM state is one of `ready`, `loading`, `stopping`, `running`, `halted`, or `failed`. Runtime lifecycle callbacks determine running/halted completion. A fulfilled boot or reboot request alone is not the completion signal. Active callbacks update controls, resize the guest console, and focus the terminal when execution starts.

Reboot first flushes editor changes, clears pending input, changes state to `stopping`, and requests an orderly guest reboot. While stopping, the button reads `Reset VM`. A reset callback establishes that the new boot has begun. Orderly reboot preserves both workspace files and guest root-disk changes.

Reset during stopping invokes recovery without flushing the editor. Recovery disables editing and example selection, force-halts the guest, snapshots/restores the selected workspace through a cold switch, discards root-disk overlays, and boots again. Buffered editor text stays with its owner and becomes writable again afterward. Preventing pointer-induced blur on the reset button is part of preserving that unflushed text.

Errors retire pending input, set state to `failed`, and report the failure. Runtime destruction unsubscribes from filesystem events and invalidates callbacks before halting/destroying the machine. A delayed callback from a retired runtime must never mutate the replacement session.


Terminal input and output
-------------------------

User terminal input waits for the editor flush before entering the VM input queue. `sendInput` copies bytes and captures its input generation before that wait. Switches, resets, halts, reboot requests, and failures retire input generations; stale input cannot reach the next workspace or boot.

`TerminalInputQueue` preserves FIFO order and advances only by the byte count accepted by the runtime. Partial acceptance keeps the remainder queued; bounded draining and timed retries yield to guest execution. Invalid acceptance counts fail explicitly. Protocol replies bypass editor flushing and request immediate draining of the same queue, preserving earlier queued input order while avoiding guest query timeouts.

The terminal uses xterm with fit and optional WebGL rendering; unavailable WebGL falls back to DOM rendering. Terminal reset replaces the terminal/parser instance so pending output from the retired instance cannot reappear. Font sizing comes from the page's computed root size at startup, with browser zoom retained. Resizes propagate to the guest console once the machine has started.

The console is a terminal emulator with cursor addressing, colors, normal/alternate screens, scrollback, text selection, and clipboard paste. The host surface and terminal background are pure black, including unused padding. Connected box-drawing glyphs must meet at cell boundaries so the debugger's pane borders do not develop gaps at different zoom levels. GPU rendering is optional; rendering failure must retain a usable DOM console rather than block boot or lose input.

Fit the grid only when its container has nonzero dimensions, after the monospace font is ready, on visible-pane resize, and when VM becomes active. A hidden tab must not force an unusable zero-size grid into the guest. Runtime start/reset completion fits the terminal, sends its columns/rows, and focuses input. Preserve the grid dimensions and prior focus where applicable when replacing the terminal on reset.

Text input is UTF-8; terminal binary input preserves byte values without a second text encoding. Paste clears selection and returns to the bottom of scrollback. Respect bracketed-paste mode; in that mode strip embedded ESC characters from pasted text so it cannot inject an early paste terminator. Keyboard input and paste use the same flush-before-input and generation rules. Large pastes must survive partial VM acceptance without loss, duplication, mutation by the caller, or reordering.

Terminal-generated replies to cursor/size queries are not user commands. They must bypass editor flushing and attempt prompt FIFO delivery so full-screen programs do not time out waiting for a response. A reply stays behind bytes already queued; it must not reorder that queue. The current integration explicitly distinguishes user input from parser replies and fails if that distinction is unavailable, rather than silently routing replies through a slow user-input path.

Reset must retire both queued guest output awaiting parsing and delayed input awaiting a flush. Clearing visible cells alone is insufficient: an old prompt or debugger screen must never reappear after the new boot screen. Disposal also releases event subscriptions, resize observation, and renderer resources.


Instructions and failures
-------------------------

Documentation reads from the current workspace, so guest/editor changes can update instructions. CommonMark safe rendering filters raw HTML and unsafe link protocols. Workspace images become data URLs; SVG remains in the browser's image context rather than becoming active page markup.

Image paths are tracked as dependencies even when loading fails. Missing or unsupported images produce a pane-local error, and later filesystem changes can repair the pane without restarting the app. Documentation rendering errors must not prevent file selection, example switching, or VM operation.

Resolve workspace-relative images against the documentation file's directory, preserving nested paths. Supported local image types are GIF, JPEG, PNG, WebP, and SVG. Safe Markdown still provides headings, paragraphs, lists, code blocks, and links. README scrolls independently from the editor and terminal. If its configured file disappears, hide its tab; if that tab was active, switch to VM. Do not treat a missing image as if the documentation file or whole example were absent.


Publication and audit boundaries
--------------------------------

The release workflow tests and builds binaries before tagging and publishing the selected core version. Only successful publication starts the automatic demo workflow. Manual demo builds use the published release matching the checked-in Cargo version; they do not publish or compile a new guest Risclet release.

`.github/workflows/demo.yml` requires the matching tag and published binary asset, runs `make -C demo test`, uploads the complete site artifact, and checks freshness before Pages deployment. Automatic and manual entries share the Pages concurrency group. The binary version must still match main; manual builds must also still match main's current commit. Superseded builds skip publication. Demo failure does not retract a successful binary release.

Regression coverage in `ui/tests/` and `tests/` exercises queued input, editor conflicts, namespace restoration, stale selections, recovery, rendering, startup, and deployment assets. Browser checks use temporary Chrome profiles, headed with `DISPLAY` and headless otherwise, and close their test processes. Changes across these boundaries must preserve namespace isolation, flush-before-user-input ordering, lifecycle completion signals, and compatibility between deployed asset URLs and their bytes.


Behavioral acceptance scenarios
-------------------------------

### Editing, selection, and storage

| Scenario                                                                | Required observation                                                                                      |
|-------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------|
| Edit assembly and immediately type a terminal command                   | The guest sees the new text before receiving command bytes                                                |
| Switch files/examples while the editor write fails                      | The failure remains visible, dirty text survives, and selection does not advance through the failed flush |
| Guest renames a dirty file or its parent directory                      | The buffer follows the new path; later flush writes there                                                 |
| Guest changes a dirty file, student keeps/discards edits                | Keep preserves the buffer for replacement on flush; discard loads current filesystem contents             |
| Create binary files, empty files, links, and metadata; switch away/back | The complete namespace is restored, not just the originally packaged source files                         |

### Asynchronous operations and recovery

| Scenario                                                          | Required observation                                                                                            |
|-------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------|
| Rapidly select A, B, then C while preparation is pending          | Only the current request publishes its views; no stale workspace or delayed input appears in C                  |
| Reboot request resolves before runtime reset callback             | Control still offers Reset; running completion waits for the callback                                           |
| Force reset with unflushed text and guest-created workspace files | Both survive; root-disk overlay changes are discarded; no blur-triggered write commits the buffer during reset  |
| First WASM fetch fails, then retry succeeds                       | One coherent error/retry flow reaches a populated, usable workspace and guest                                   |
| Boot begins during background disk prefetch                       | Prefetch aborts and never resumes in that page session; boot does not wait for the remaining speculative blocks |

### Visible interface and terminal

| Scenario                                                        | Required observation                                                                                  |
|-----------------------------------------------------------------|-------------------------------------------------------------------------------------------------------|
| Open at wide/narrow viewport sizes, resize, switch tabs         | Initial proportions and user-adjusted splits remain stable; scrolling stays within panes              |
| Run the core debugger, resize the terminal, use arrows and help | Full terminal behavior reaches the core; colors, connected borders, keys, and alternate screen work   |
| Paste more input than one FIFO can accept                       | Every intended byte arrives once in order, after editor flush, without freezing UI progress           |
| Queue old terminal output, then reset                           | Retired output never reappears; parser replies do not wait for unrelated editor flushing              |
| Lose WebGL or repair a missing README image                     | Fallback console remains usable; an image repair updates only the pane without losing editor/VM state |
