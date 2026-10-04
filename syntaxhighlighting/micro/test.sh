#!/usr/bin/env bash
set -euo pipefail

# Keep the test dependency and its build files outside the assembler project.
project_dir=$(cd -- "$(dirname -- "$0")/.." && pwd)
check_dir=$(mktemp -d)
trap 'rm -rf -- "$check_dir"' EXIT
cp "$project_dir/micro/test.go" "$check_dir/main.go"
cat > "$check_dir/go.mod" <<'MODULE'
module risclet-micro-check

go 1.16

require github.com/zyedidia/micro/v2 v2.0.14
MODULE

# Compile against the same highlighting engine as the supported micro release.
(
    cd "$check_dir"
    go mod tidy
    go build -o "$check_dir/check" .
)
cd "$project_dir"
"$check_dir/check"
