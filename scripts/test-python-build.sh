#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV_DIR="${ROOT_DIR}/.venv"

cd "${ROOT_DIR}"

if [[ ! -d "${VENV_DIR}" ]]; then
  python -m venv "${VENV_DIR}"
fi

# shellcheck source=/dev/null
source "${VENV_DIR}/bin/activate"

python -m pip install --upgrade pip
python -m pip install -r requirements-dev.txt

# Build a fresh wheel from the current Rust + Python sources.
python -m maturin build --release -o dist

# Install the newly built wheel so pytest imports the local build artifacts.
python -m pip install --force-reinstall dist/render_slides-*.whl

pytest -q
