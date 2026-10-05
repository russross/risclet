Risclet browser demo
====================

Build and test from the repository root:

```sh
make -C demo
make -C demo test
python3 -m http.server --directory demo/dist 8000
```

Open <http://localhost:8000/>. `make -C demo` is the primary entrypoint;
its prerequisites download releases, install the locked UI dependencies, build
the guest filesystem, compile the client, and assemble the static site.
Incremental builds reuse unchanged downloads and outputs.

Build inputs
------------

The build requires GNU Make, Node.js 22 with npm, uv, curl, tar, gzip, cpio,
fakeroot, QEMU's `qemu-system-riscv64`, and `mkfs.erofs` with tar input support.
Tests use Google Chrome with a temporary profile and close it afterward. They
run headed when `DISPLAY` is set and headless otherwise.

`version` selects the published riscbox release, initially `2026.9.33`.
Risclet's version defaults to `../Cargo.toml`. Both downloads must already be
published; to work against a different published Risclet version, use:

```sh
make -C demo RISCLET_VERSION=0.4.9
```

The release downloader verifies GitHub's SHA-256 asset digest. Downloads are
cached by version under `build/downloads/`; the selected versions are recorded
in `build/versions` and deployed as `versions.txt`. `make -C demo clean` removes
generated images, downloads, site files, and test fixtures.

Riscbox integration
-------------------

The build follows riscbox's [release HOWTO](https://github.com/russross/riscbox/blob/main/HOWTO.md)
and [demo](https://github.com/russross/riscbox/tree/main/demo). The extracted
release is copied unchanged to `dist/riscbox/`, including its documentation,
types, network modules, firmware, kernel, runtime, and disk splitter. No local
riscbox checkout or separately installed riscbox modules are required.

The image build uses the release's firmware and kernel to boot a verified Alpine
minirootfs in QEMU. `guest/sbin/demo-prepare` installs Vim, Micro, and the selected
published Risclet binary, then exports an EROFS image. The release's executable
`splitimg.py` produces content-named HTTP disk chunks. The guest uses a read-only
root disk, temporary writable overlays, and an uncached 9p mount at
`/home/risclet` so editor writes are immediately visible to guest processes.

Micro uses `/etc/micro`, with bundled syntax definitions in the base image and
a writable overlay owned by the demo user. Settings and history stay outside
the 9p workspace and reset on reboot; the overlay stores changes under `/run`.

The browser uses `Riscbox.instantiate`, `loadResolvedConfig`, `prepareResolved`,
`filesystem`, and `block`. Lifecycle completion comes from runtime callbacks;
acceptance of a reboot request does not mean the guest has rebooted. Terminal
resize and input calls run only while the runtime is started. Cold resets retire
pending disk reads before clearing overlays and replacing the workspace.

Page-linked scripts and styles retain regular names. Riscbox assets retain their
upstream filenames; its configuration loader uses `no-store`, and the client
revalidates the WASM response. Boot payloads and split disks retain the hashes
assigned by riscbox. The demo's compressed source bundle and dynamically loaded
UI assets use content hashes.

Sources and session behavior
----------------------------

`examples/examples.json` declares the existing source trees. `bundle.mjs` packs
all declared files, including binary bytes, into one compressed JSON download.
The browser validates and loads that bundle once before selection. There are no
per-file source requests. See [examples/README.md](examples/README.md) for the
manifest format; creating the replacement demo trees is separate work.

Each example owns an in-memory snapshot of its complete 9p namespace. Switching
flushes the editor, force-halts the VM, captures that namespace, cold-resets the
VM and disk overlays, and restores the next example. Instructions open first;
selecting the VM tab boots it. Examples without instructions boot immediately.

Reboot VM requests an orderly guest reboot while preserving the workspace. The
button reads Reset VM until the reboot callback arrives. Reset VM forces
recovery, retaining workspace files and unflushed editor text. Refreshing the
page starts a new session. Editor text flushes to the VM
workspace on blur, terminal interaction, example switches, and after 30 seconds
of inactivity; it does not persist across refreshes.

Release deployment
------------------

The existing version/tag/release jobs run first. After publication, `build_demo`
downloads the exact new RISC-V Linux binary, builds and tests the site, and uploads
the Pages artifact. `deploy_demo` publishes it to
<https://russross.github.io/risclet/>. The repository's Pages source must be set to
GitHub Actions. A failed demo build leaves the published release intact and does
not deploy a partial site; the failed job can be retried.

The pinned Wterm build has guarded patches for viewport alignment, box drawing,
and a separate protocol-response callback. Automatic terminal-query replies must
reach the guest without being treated as user input that flushes editor changes.
Webpack currently reports its default size warnings for the editor/terminal
bundle and terminal WASM; those warnings are not disabled.
