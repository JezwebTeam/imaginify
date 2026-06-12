#!/usr/bin/env bash
# ----------------------------------------------------------------
# Imaginify - macOS/Linux launcher
# Creates a local virtual environment, installs dependencies, then
# runs the app from source.
# ----------------------------------------------------------------
set -euo pipefail
cd "$(dirname "$0")"

echo "=== Imaginify source launcher ==="
echo

if ! command -v python3 >/dev/null 2>&1; then
    echo "Python 3 not found."
    echo "Install Python 3.10+ from https://www.python.org/downloads/macos/"
    echo "or run:  brew install python"
    exit 1
fi

python3 -c "import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)" || {
    echo "Python 3.10+ is required."
    echo "Current version: $(python3 --version)"
    exit 1
}

if [ ! -x ".venv/bin/python" ]; then
    echo "Creating local virtual environment..."
    python3 -m venv .venv
    echo
fi

PYTHON=".venv/bin/python"
if ! "$PYTHON" -c "import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)"; then
    echo "The existing .venv uses an unsupported Python version."
    echo "Remove .venv and rerun this script with Python 3.10+."
    exit 1
fi

echo "Installing dependencies..."
"$PYTHON" -m pip install --upgrade pip
"$PYTHON" -m pip install -r requirements.txt
echo

echo "Verifying GUI dependencies..."
if ! "$PYTHON" -c "import tkinter; import customtkinter; import darkdetect; from PIL import ImageTk"; then
    echo "Tkinter/CustomTkinter validation failed."
    echo "Use a Tk-enabled Python install, such as python.org Python for macOS."
    echo "If you use Homebrew, make sure Tcl/Tk support is installed and available."
    exit 1
fi
echo

echo "Starting Imaginify..."
exec "$PYTHON" imaginify.py
