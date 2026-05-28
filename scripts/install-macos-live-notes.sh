#!/usr/bin/env bash
set -euo pipefail

BRANCH="${MEETILY_BRANCH:-ivnhq/live-notes-roadmap}"
REPO_URL="${MEETILY_REPO_URL:-https://github.com/Ivnhq/meetily.git}"
INSTALL_DIR="${MEETILY_INSTALL_DIR:-$HOME/Applications/Meetily Live Notes Source}"
APP_INSTALL_PATH="${MEETILY_APP_INSTALL_PATH:-$HOME/Applications/Meetily Live Notes.app}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This installer must be run on macOS."
  exit 1
fi

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "This helper targets Apple Silicon Macs. Intel Macs are not supported by the current macOS workflow."
  exit 1
fi

echo "Meetily Live Notes macOS installer"
echo "Branch: $BRANCH"
echo "Install source: $INSTALL_DIR"
echo "Install app: $APP_INSTALL_PATH"
echo ""

if ! xcode-select -p >/dev/null 2>&1; then
  echo "Installing Apple command line tools. Finish the macOS prompt, then rerun this command."
  xcode-select --install || true
  exit 1
fi

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew is required. Installing Homebrew first."
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

  if [[ -x /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
  fi
fi

if [[ -x /opt/homebrew/bin/brew ]]; then
  eval "$(/opt/homebrew/bin/brew shellenv)"
fi

echo "Installing required build tools."
brew install git node rustup cmake ollama

if ! command -v cargo >/dev/null 2>&1; then
  RUSTUP_BIN=""
  if command -v rustup >/dev/null 2>&1; then
    RUSTUP_BIN="$(command -v rustup)"
  elif [[ -x "$(brew --prefix rustup)/bin/rustup" ]]; then
    RUSTUP_BIN="$(brew --prefix rustup)/bin/rustup"
  fi

  if [[ -n "$RUSTUP_BIN" ]]; then
    "$RUSTUP_BIN" default stable
  else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --profile minimal --default-toolchain stable
  fi

  if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
  fi
fi

if ! command -v cargo >/dev/null 2>&1 && [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck source=/dev/null
  source "$HOME/.cargo/env"
fi

mkdir -p "$(dirname "$INSTALL_DIR")"

if [[ -d "$INSTALL_DIR/.git" ]]; then
  echo "Updating existing checkout."
  git -C "$INSTALL_DIR" fetch origin "$BRANCH"
  git -C "$INSTALL_DIR" checkout "$BRANCH"
  git -C "$INSTALL_DIR" pull --ff-only origin "$BRANCH"
else
  echo "Cloning Meetily."
  git clone --branch "$BRANCH" "$REPO_URL" "$INSTALL_DIR"
fi

cd "$INSTALL_DIR"

echo "Installing frontend dependencies."
cd frontend
npm install
cd ..

echo "Building local LLM helper with Metal acceleration."
cargo build --release -p llama-helper --features metal
mkdir -p frontend/src-tauri/binaries
cp target/release/llama-helper frontend/src-tauri/binaries/llama-helper-aarch64-apple-darwin

echo "Building Meetily macOS app."
cd frontend
TAURI_APP_BUNDLE_CONFIG="$(mktemp)"
trap 'rm -f "$TAURI_APP_BUNDLE_CONFIG"' EXIT
printf '%s\n' '{"bundle":{"targets":["app"],"createUpdaterArtifacts":false}}' > "$TAURI_APP_BUNDLE_CONFIG"
npx tauri build --config "$TAURI_APP_BUNDLE_CONFIG" -- --features metal
cd ..

DMG_PATH="$(find target frontend/src-tauri/target -path "*/bundle/dmg/*.dmg" -type f 2>/dev/null | head -1)"
APP_PATH="$(find target frontend/src-tauri/target -path "*/bundle/macos/*.app" -type d 2>/dev/null | head -1)"

if [[ -n "$DMG_PATH" ]]; then
  echo ""
  echo "Build complete: $DMG_PATH"
  open -R "$DMG_PATH"
  open "$DMG_PATH"
elif [[ -n "$APP_PATH" ]]; then
  echo ""
  echo "Installing app to: $APP_INSTALL_PATH"
  mkdir -p "$(dirname "$APP_INSTALL_PATH")"
  rm -rf "$APP_INSTALL_PATH"
  ditto "$APP_PATH" "$APP_INSTALL_PATH"
  mdimport "$APP_INSTALL_PATH" >/dev/null 2>&1 || true
  echo "Build complete: $APP_INSTALL_PATH"
  open -R "$APP_INSTALL_PATH"
  open "$APP_INSTALL_PATH"
else
  echo "Build finished, but no DMG or app bundle was found."
  exit 1
fi

echo ""
echo "If macOS warns that the app cannot be opened, right-click the app and choose Open."
