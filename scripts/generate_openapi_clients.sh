#!/usr/bin/env nix-shell
#! nix-shell -i bash --pure
#! nix-shell -p bash openapi-generator-cli git
#! nix-shell -I nixpkgs=https://github.com/NixOS/nixpkgs/archive/refs/tags/23.11.tar.gz

if [ -z "$1" ]; then
  echo "Error: No input JSON file provided."
  exit 1
fi

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
ROOT_DIR=$(dirname "$SCRIPT_DIR")

echo "Generating client for json: $1"

openapi-generator-cli generate \
  --input-spec "$1" \
  --generator-name rust \
  --package-name cid-router-client \
  --output "$ROOT_DIR/openapi-client"

