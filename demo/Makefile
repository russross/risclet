# Local builds consume published releases; deployment supplies an explicit version.
RISCBOX_VERSION := $(strip $(shell cat version))
RISCLET_VERSION ?= $(shell sed -n 's/^version = "\(.*\)"/\1/p' ../Cargo.toml)
ALPINE_VERSION := 3.24.2
ALPINE_SHA256 := 57132e6e4f3a4ba9ffdf24e485513ce507e45471e7fd565aeed91c055cd63f7b
RISCBOX_ARCHIVE := build/downloads/riscbox-$(RISCBOX_VERSION).tar.gz
RISCLET_BINARY := build/downloads/risclet-$(RISCLET_VERSION)
ALPINE_ARCHIVE := build/downloads/alpine-minirootfs-$(ALPINE_VERSION)-riscv64.tar.gz
UI_SOURCES := $(shell find ui -maxdepth 1 -type f | sort) ../syntaxhighlighting/codemirror/risclet.ts
GUEST_SOURCES := $(shell find guest -type f | sort)
EXAMPLE_SOURCES := $(shell find examples -type f | sort)

all: dist/.built

# Compare input inventories so deletions and version overrides invalidate only their consumers.
build/ui-inputs: FORCE
	@mkdir -p build
	@sha256sum $(UI_SOURCES) > $@.part
	@cmp -s $@.part $@ && rm $@.part || mv $@.part $@

build/guest-inputs: FORCE
	@mkdir -p build
	@sha256sum $(GUEST_SOURCES) > $@.part
	@cmp -s $@.part $@ && rm $@.part || mv $@.part $@

build/example-inputs: FORCE
	@mkdir -p build
	@sha256sum $(EXAMPLE_SOURCES) > $@.part
	@cmp -s $@.part $@ && rm $@.part || mv $@.part $@

build/riscbox-version: FORCE
	@mkdir -p build
	@printf '%s\n' '$(RISCBOX_VERSION)' > $@.part
	@cmp -s $@.part $@ && rm $@.part || mv $@.part $@

build/risclet-version: FORCE
	@mkdir -p build
	@printf '%s\n' '$(RISCLET_VERSION)' > $@.part
	@cmp -s $@.part $@ && rm $@.part || mv $@.part $@

build/versions: build/riscbox-version build/risclet-version
	@printf 'riscbox=%s\nrisclet=%s\n' '$(RISCBOX_VERSION)' '$(RISCLET_VERSION)' > $@.part
	@mv $@.part $@

$(RISCBOX_ARCHIVE): scripts/download-release.sh scripts/fetch.sh
	./scripts/download-release.sh riscbox '$(RISCBOX_VERSION)' '$@'

$(RISCLET_BINARY): scripts/download-release.sh scripts/fetch.sh
	./scripts/download-release.sh risclet '$(RISCLET_VERSION)' '$@'

$(ALPINE_ARCHIVE): scripts/fetch.sh
	./scripts/fetch.sh 'https://dl-cdn.alpinelinux.org/alpine/v$(basename $(ALPINE_VERSION))/releases/riscv64/$(notdir $@)' '$(ALPINE_SHA256)' '$@'

# Extract atomically; public declarations and runtime bytes always come from one release.
build/riscbox.stamp: $(RISCBOX_ARCHIVE) build/riscbox-version
	rm -rf build/riscbox.part
	mkdir -p build/riscbox.part
	tar -xzf '$(RISCBOX_ARCHIVE)' --strip-components=1 -C build/riscbox.part
	test -f build/riscbox.part/riscbox.d.ts
	test -f build/riscbox.part/riscbox.js
	test -f build/riscbox.part/riscbox.wasm
	rm -rf build/riscbox
	mv build/riscbox.part build/riscbox
	touch $@

ui/node_modules/.installed: ui/package.json ui/package-lock.json
	cd ui && npm ci
	touch $@

build/ui/.built: build/ui-inputs ui/node_modules/.installed build/riscbox.stamp
	cd ui && npm run check && npm run build
	touch $@

build/rootfs.ext4: build/guest-inputs build-image.sh $(RISCLET_BINARY) $(ALPINE_ARCHIVE) build/risclet-version
	./build-image.sh '$(RISCLET_BINARY)' '$(ALPINE_ARCHIVE)'

# Disk splitting is independent of UI, styles, and example source changes.
build/disk-name dist/.disk-built &: build/rootfs.ext4 build/riscbox.stamp Makefile
	mkdir -p dist
	python3 build/riscbox/splitimg.py build/rootfs.ext4 dist > build/disk-name.part
	awk '{print $$1}' build/disk-name.part > build/disk-name
	rm build/disk-name.part
	touch dist/.disk-built

build/examples.json.gz: build/example-inputs scripts/bundle.py
	python3 scripts/bundle.py examples $@

build/riscbox.cfg: build/disk-name build/riscbox-version scripts/site.py
	python3 scripts/site.py config '$(RISCBOX_VERSION)'
	@touch $@

# Published assets retain immutable filenames so old pages cannot fetch new incompatible bytes.
dist/riscbox-$(RISCBOX_VERSION)/.built: build/riscbox.stamp
	mkdir -p '$(@D)'
	cp -R build/riscbox/. '$(@D)/'
	touch $@

dist/.ui-built: build/ui/.built
	mkdir -p dist
	cp build/ui/app-* dist/
	touch $@

dist/favicon.svg: web/favicon.svg
	mkdir -p dist
	cp $< $@

dist/versions.txt: build/versions
	mkdir -p dist
	cp $< $@

dist/.nojekyll:
	mkdir -p dist
	touch $@

dist/.built: scripts/site.py web/index.html web/style.css build/examples.json.gz build/riscbox.cfg dist/.disk-built dist/.ui-built dist/riscbox-$(RISCBOX_VERSION)/.built dist/favicon.svg dist/versions.txt dist/.nojekyll
	python3 scripts/site.py publish '$(RISCBOX_VERSION)'
	touch $@

check: ui/node_modules/.installed build/riscbox.stamp
	cd ui && npm run check

# Browser tests use Chrome with disposable profiles, headed when DISPLAY is set.
test: all
	cd ui && npm test
	node --experimental-websocket --test tests/*.test.mjs

clean:
	rm -rf build dist ui/build

FORCE:
.PHONY: all check test clean FORCE
.DELETE_ON_ERROR:
