Risclet examples
================

Select a demo and read its README for commands and debugger exercises:

*   `sort`: insertion sort, observing individual shifts and complete insertions.
*   `quicksort`: byte-array partitioning and recursive completion points.
*   `binary-search`: contracting bounds and whole query calls.
*   `guess`: an interactive guessing game, then replay of captured input.

Each directory runs directly with `risclet` or `risclet debug --strict`.
`start.s` drives the demo and exits; functions declare their argument counts.
There are no Makefiles or deliberately incomplete student solutions.

`examples.json` lists each example's ID, display title, preferred editable file,
documentation path, and complete set of relative workspace file paths.
The source manifest contains paths only; the build reads those paths and packs
all examples into one `examples-HASH.json.gz` asset.

The bundle has a versioned JSON envelope and base64-encoded file bodies with
byte lengths. This preserves binary and empty files as well as nested paths.
The browser validates the entire bundle before creating a workspace and performs
no per-file fetches. The bundle's content hash changes when any packaged source
or description changes.
