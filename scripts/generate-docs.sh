#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${ROOT_DIR}"

# Build crate documentation (including private items) without dependency docs
# to keep generation quick and focused on this project.
cargo doc --no-deps --document-private-items

DOC_INDEX="${ROOT_DIR}/target/doc/render_slides/index.html"
echo "Documentation generated at: ${DOC_INDEX}"
