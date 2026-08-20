#!/usr/bin/env bash
set -euo pipefail

BRANCH="${MEETILY_BRANCH:-codex/meetily-all-phases}"
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

echo "Checking required build tools."
MISSING_FORMULAE=()
command -v git >/dev/null 2>&1 || MISSING_FORMULAE+=(git)
command -v node >/dev/null 2>&1 || MISSING_FORMULAE+=(node)
command -v pnpm >/dev/null 2>&1 || MISSING_FORMULAE+=(pnpm)
if ! command -v cargo >/dev/null 2>&1 && [[ ! -x "$HOME/.cargo/bin/cargo" ]]; then
  MISSING_FORMULAE+=(rustup)
fi
command -v cmake >/dev/null 2>&1 || MISSING_FORMULAE+=(cmake)
command -v ollama >/dev/null 2>&1 || MISSING_FORMULAE+=(ollama)

if [[ "${#MISSING_FORMULAE[@]}" -gt 0 ]]; then
  echo "Installing missing build tools: ${MISSING_FORMULAE[*]}"
  HOMEBREW_NO_AUTO_UPDATE=1 brew install "${MISSING_FORMULAE[@]}"
else
  echo "Required build tools are already installed."
fi

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
pnpm install --frozen-lockfile
cd ..

echo "Building local LLM helper with Metal acceleration."
cargo build --release -p llama-helper --features metal
mkdir -p frontend/src-tauri/binaries
cp target/release/llama-helper frontend/src-tauri/binaries/llama-helper-aarch64-apple-darwin

echo "Building the read-only Meetily CLI."
TAURI_CONFIG='{"bundle":{"externalBin":["binaries/llama-helper","binaries/ffmpeg"]}}' \
  cargo build --release -p meetily --bin meetily-cli --features metal
cp target/release/meetily-cli frontend/src-tauri/binaries/meetily-cli-aarch64-apple-darwin

echo "Building Meetily macOS app."
cd frontend
TAURI_APP_BUNDLE_CONFIG="$(mktemp)"
trap 'rm -f "$TAURI_APP_BUNDLE_CONFIG"' EXIT
printf '%s\n' '{"bundle":{"targets":["app"],"createUpdaterArtifacts":false},"plugins":{"updater":{"endpoints":["https://github.com/Ivnhq/meetily/releases/latest/download/latest.json"]}}}' > "$TAURI_APP_BUNDLE_CONFIG"
NEXT_PUBLIC_MEETILY_DISABLE_UPDATES=1 \
  npx tauri build --config "$TAURI_APP_BUNDLE_CONFIG" -- --features metal
cd ..

find_first_bundle() {
  local bundle_pattern="$1"
  local bundle_type="$2"
  local search_root
  local result

  for search_root in "$INSTALL_DIR/target" "$INSTALL_DIR/frontend/src-tauri/target"; do
    if [[ -d "$search_root" ]]; then
      result="$(find "$search_root" -path "$bundle_pattern" -type "$bundle_type" -print -quit)"
      if [[ -n "$result" ]]; then
        printf '%s\n' "$result"
        return 0
      fi
    fi
  done

  return 0
}

DMG_PATH="$(find_first_bundle "*/bundle/dmg/*.dmg" f)"
APP_PATH="$(find_first_bundle "*/bundle/macos/*.app" d)"

if [[ -n "$APP_PATH" ]]; then
  BUNDLE_EXECUTABLE="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP_PATH/Contents/Info.plist")"
  if [[ "$BUNDLE_EXECUTABLE" != "meetily" || ! -x "$APP_PATH/Contents/MacOS/meetily" ]]; then
    echo "Built app has an invalid main executable: $BUNDLE_EXECUTABLE"
    exit 1
  fi
fi

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
