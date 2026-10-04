# The demo always consumes published releases, including during local builds.
RISCBOX_VERSION := $(strip $(shell cat version))
RISCLET_VERSION ?= $(shell sed -n 's/^version = "\(.*\)"/\1/p' ../Cargo.toml)
RISCBOX_ARCHIVE := build/downloads/riscbox-$(RISCBOX_VERSION).tar.gz
RISCLET_BINARY := build/downloads/risclet-$(RISCLET_VERSION)
UI_SOURCES := $(shell find ui -type f ! -path 'ui/node_modules/*' ! -path 'ui/build/*')
GUEST_SOURCES := $(shell find guest -type f)
EXAMPLE_SOURCES := $(shell find examples -type f)
WEB_SOURCES := $(shell find web -type f)

all: dist/.built

# Record overrides as a real prerequisite, without rebuilding on identical input.
build/versions: FORCE
	@mkdir -p build
	@printf 'riscbox=%s\nrisclet=%s\n' '$(RISCBOX_VERSION)' '$(RISCLET_VERSION)' > $@.part
	@cmp -s $@.part $@ && rm $@.part || mv $@.part $@

$(RISCBOX_ARCHIVE): download.mjs
	node download.mjs riscbox '$(RISCBOX_VERSION)' '$@'

$(RISCLET_BINARY): download.mjs
	node download.mjs risclet '$(RISCLET_VERSION)' '$@'

# Public declarations and runtime bytes come from the same extracted archive.
build/riscbox.stamp: $(RISCBOX_ARCHIVE) build/versions Makefile
	rm -rf build/riscbox
	mkdir -p build/riscbox
	tar -xzf '$(RISCBOX_ARCHIVE)' --strip-components=1 -C build/riscbox
	test -f build/riscbox/riscbox.d.ts
	test -f build/riscbox/riscbox.js
	test -f build/riscbox/riscbox.wasm
	touch $@

ui/node_modules/.installed: ui/package.json ui/package-lock.json
	cd ui && npm ci
	touch $@

build/ui/.built: $(UI_SOURCES) ui/node_modules/.installed build/riscbox.stamp
	cd ui && npm run check && npm run build
	touch $@

build/rootfs.erofs: $(GUEST_SOURCES) build-image.sh $(RISCLET_BINARY) build/riscbox.stamp build/versions Makefile
	./build-image.sh build/riscbox '$(RISCLET_BINARY)'

# The site stamp is written only after all runtime, disk, and example assets exist.
dist/.built: assemble.mjs bundle.mjs $(WEB_SOURCES) $(EXAMPLE_SOURCES) build/ui/.built build/rootfs.erofs build/versions
	node assemble.mjs
	touch $@

check: ui/node_modules/.installed build/riscbox.stamp
	cd ui && npm run check

# Browser tests use Chrome with a disposable profile, headed when DISPLAY is set.
test: all
	cd ui && RISCBOX_CLIENT_RUNTIME=../build/riscbox/riscbox.js RISCBOX_CLIENT_WASM=../build/riscbox/riscbox.wasm npm test
	node --experimental-websocket --test tests/*.test.mjs

clean:
	rm -rf build dist ui/build

FORCE:
.PHONY: all check test clean FORCE
.DELETE_ON_ERROR:
