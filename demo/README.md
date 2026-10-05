Risclet browser demo
====================

Build and test from the repository root:

```sh
make -C demo
make -C demo test
python3 -m http.server --directory demo/dist 8000
```

Open <http://localhost:8000/>. Make downloads published releases, installs the
locked UI dependencies, constructs the guest disk, compiles the client, and
publishes the static files. It tracks those outputs independently: changing
examples or page styles does not rebuild the VM image or compile the UI; changing
UI tests does not rebuild the production bundle. Interrupted builds resume from
completed prerequisites. Input inventories detect deleted source files as well
as modified ones. `make -j` can run independent steps concurrently.

Build inputs
------------

The build requires GNU Make 4.3 or newer, Node.js 22 with npm, Python 3.10 or newer,
curl, jq, sha256sum, tar, fakeroot, and e2fsprogs (`mkfs.ext4`). Python scripts use
only the standard library and run directly through `python3`. No uv environment,
Python package installation, or local riscbox checkout is needed.

Tests use Google Chrome with disposable profiles and close it afterward. They
run headed when `DISPLAY` is set and headless otherwise. `make -C demo test` is
the complete validation entry point; direct `npm test` in `demo/ui` requires the
published runtime to have been extracted by Make first and fails if it is absent.

`version` selects the published riscbox release. Risclet's version defaults to
`../Cargo.toml`. Both downloads must already be published; to select a different
published Risclet version, use:

```sh
make -C demo RISCLET_VERSION=0.4.12
```

Downloads are checked against GitHub's SHA-256 asset digests, with bounded network
retries. An interrupted or invalid download never replaces the final cached file.
The Alpine minirootfs has a pinned version and SHA-256 digest. Downloads are cached
by version under `build/downloads/`; selected versions are recorded in
`build/versions` and deployed as `versions.txt`. `make -C demo clean` removes all
generated images, downloads, site files, and browser fixtures.

Riscbox integration
-------------------

The build follows riscbox's [release HOWTO](https://github.com/russross/riscbox/blob/main/HOWTO.md)
and [demo](https://github.com/russross/riscbox/tree/main/demo). The extracted
release is copied unchanged into a versioned `dist/riscbox-VERSION/` directory,
including its documentation, declarations, runtime, firmware, kernel, and disk
splitter. Public TypeScript declarations and runtime bytes come from that same
release.

Image construction extracts the verified Alpine minirootfs, adds the selected
published Risclet binary and guest configuration, and populates a 16 MiB ext4
image with `mkfs.ext4 -d` under fakeroot. It does not boot a VM or install additional
Alpine packages. The dependency-free upstream `splitimg.py` runs through `python3`
and produces content-named 256 KiB HTTP disk chunks. Splitting is a separate Make target
and does not run again when only the UI or examples change.

The guest mounts its ext4 root read-write and mounts uncached 9p named `shared`
at `/home/risclet`, so editor writes are immediately visible to guest processes.
Guest filesystem changes survive orderly reboots; cold resets discard the
runtime's disk changes. Runtime callbacks establish lifecycle completion;
acceptance of a reboot request does not mean the guest has rebooted.

The app, styles, configuration, and compressed examples have content-addressed
filenames. Runtime paths include the pinned release version; firmware and disk
chunks retain upstream content names. Publishing page changes cannot replace
bytes at an existing asset URL. Local incremental builds retain older immutable
assets; clean builds start a fresh deployment artifact.

Sources and widget boundaries
-----------------------------

`examples/examples.json` declares the source trees. `scripts/bundle.py` validates
and packs all declared files, including binary bytes, into one compressed JSON
download. The browser validates and loads it once before selection. There are no
per-file source requests. See [examples/README.md](examples/README.md) for the
manifest format.

`ui/index.ts` coordinates selection, tabs, editor flushing, and terminal input.
The terminal owns rendering and input events; the editor owns its buffered text,
filesystem writes, and conflict decisions; the file tree receives paths and emits
selections; the README pane owns safe Markdown rendering, image dependencies,
and local errors. `VmSession` owns one fixed machine, its input queue, and workspace
switch/reboot/recovery operations. Filesystem events fan out through the app.

Each example owns an in-memory snapshot of its complete 9p namespace, including
links and metadata. Switching flushes the editor, force-halts the VM, snapshots
the namespace, cold-resets the VM and disk overlays, and restores the next
example. Instructions open first; selecting the VM tab boots it. Examples without
instructions boot immediately. Superseded selections and delayed terminal input
cannot replace or write into the selected workspace.

Reboot VM requests an orderly guest reboot while preserving the workspace. The
button reads Reset VM until the reboot callback arrives. Reset VM forces recovery,
retaining workspace files and unflushed editor text. Refreshing starts a new
session. Editor text flushes on blur, terminal interaction, example switches, and
after 30 seconds of inactivity; it does not persist across refreshes.

README Markdown filters raw HTML and unsafe link protocols. Workspace images,
including SVG, render through data URLs in the browser's restricted image context.
Missing or unsupported images show an error within the pane; they do not block
file selection, example switching, or the VM. Changes to a missing image can
refresh and repair the pane without reloading the app.

The terminal uses xterm.js with its fit and WebGL addons on pure black. Both the
editor and terminal use Latin Modern Mono at the root element's computed font
size, sampled once at startup; browser zoom remains under user control. WebGL
draws connected box glyphs at device-pixel boundaries; unavailable WebGL falls
back to DOM rendering. Reset replaces the terminal and its parser queue so pending
output cannot reappear on the reboot screen. Protocol replies use the pinned
xterm core's guarded user-input signal to bypass editor flushing and attempt FIFO
delivery immediately, before the guest query timeout. Webpack's default bundle
size warnings remain enabled.

Deployment
----------

New package versions retain the existing binary build/tag/release pipeline. After
publication, the release workflow calls `.github/workflows/demo.yml` with that
exact version. The demo workflow runs the full test/build cycle before uploading
and deploying its Pages artifact. A failed test leaves the published binary
release intact and does not publish a partial site.

For checkins between versions, use GitHub **Actions → Demo Test, Build, and Deploy
→ Run workflow**, selecting **main**. This builds the checked-in demo against the
highest published Risclet semantic version with a RISC-V Linux binary, including
published prereleases when they have higher precedence. It neither builds nor
publishes a new Risclet release and does not require Cargo's checked-in version
to match the selected published binary.

Both entry points share a Pages concurrency group. Before deployment, the workflow
checks that its binary version remains the highest published version. Manual
builds also check that their commit remains the current main commit. Superseded
builds skip publication, preventing an older queued run from replacing newer
work. Failed runs can be retried from GitHub Actions.

The repository's Pages source must be set to GitHub Actions. The site is deployed
at <https://russross.github.io/risclet/>.
