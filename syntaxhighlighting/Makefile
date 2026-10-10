# Check syntax assets without building the assembler or the VM image.
check:
	python3 -c 'import json, pathlib; [json.loads(p.read_text()) for p in pathlib.Path(".").rglob("*.json")]'
	test -z "$$(gofmt -l micro/test.go)"
	node --check vscode/test.cjs
	bash -n micro/test.sh

# Test dependencies and binaries live in disposable directories outside the project.
test: check
	cd .. && vim -Nu NONE -n -es -S syntaxhighlighting/vim/test.vim
	./micro/test.sh
	@check_dir=$$(mktemp -d); trap 'rm -rf "$$check_dir"' EXIT HUP INT TERM; \
	npm install --prefix "$$check_dir" --no-audit --no-fund vscode-textmate vscode-oniguruma && \
	NODE_PATH="$$check_dir/node_modules" node vscode/test.cjs

mostlyclean:
	@:

clean: mostlyclean
	@:

.PHONY: check test mostlyclean clean
