#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
source_css=${1:-"$script_dir/../../../packages/ui/dist/a3s-ui.cdn.min.css"}
target_dir="$script_dir/../src/island/assets"
target_css="$target_dir/a3s-ui-0.3.0.min.css"
expected_sha256=25803bd741f763a5b7ed5cb4c753cad01126bb36c6d9a188fa0b781d635dde5c

if command -v shasum >/dev/null 2>&1; then
  actual_sha256=$(shasum -a 256 "$source_css" | awk '{print $1}')
else
  actual_sha256=$(sha256sum "$source_css" | awk '{print $1}')
fi

if [ "$actual_sha256" != "$expected_sha256" ]; then
  echo "a3s-ui CSS checksum mismatch: expected $expected_sha256, got $actual_sha256" >&2
  exit 1
fi

mkdir -p "$target_dir"
cp "$source_css" "$target_css"
chmod 0644 "$target_css"
