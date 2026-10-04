Risclet examples
================

`examples.json` lists each example's ID, display title, preferred editable file,
optional documentation path, and complete set of relative workspace file paths.
The source manifest contains paths only; the build reads those paths and packs
all examples into one `examples-HASH.json.gz` asset.

The bundle has a versioned JSON envelope and base64-encoded file bodies with
byte lengths. This preserves binary and empty files as well as nested paths.
The browser validates the entire bundle before creating a workspace and performs
no per-file fetches. The bundle's content hash changes when any packaged source
or description changes.

Each source tree can also run with an installed Risclet command:

```sh
cd sort
make
```

The reduction example intentionally contains an incomplete student function.
Its Makefile uses the installed binary; installation and release selection belong
to the demo build.
