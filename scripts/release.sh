#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

VERSION=$(jq -r '.version' package.json)
COMMIT=$(git rev-parse HEAD)

echo "=== Caipi Release v$VERSION ==="

# ---------------------------------------------------------------------------
# Environment
# ---------------------------------------------------------------------------

# Source .env if present (for signing keys)
if [ -f "$REPO_ROOT/.env" ]; then
  echo "Loading .env file..."
  set -a
  source "$REPO_ROOT/.env"
  set +a
fi

REQUIRED_VARS=(
  APPLE_SIGNING_IDENTITY
  APPLE_ID
  APPLE_PASSWORD
  APPLE_TEAM_ID
  TAURI_SIGNING_PRIVATE_KEY
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD
)

MISSING=()
for var in "${REQUIRED_VARS[@]}"; do
  if [ -z "${!var:-}" ]; then
    MISSING+=("$var")
  fi
done

if [ ${#MISSING[@]} -gt 0 ]; then
  echo "Error: Missing required environment variables:"
  for var in "${MISSING[@]}"; do
    echo "  - $var"
  done
  echo ""
  echo "Set them in your shell or create a .env file in the project root."
  exit 1
fi

# ---------------------------------------------------------------------------
# Step 1: Build macOS (signed + notarized)
# ---------------------------------------------------------------------------

echo ""
echo "=== Building macOS (signed + notarized) ==="
npm run tauri build -- --target aarch64-apple-darwin

echo ""
echo "=== Renaming macOS artifacts ==="
BUILD_TARGET=aarch64-apple-darwin node scripts/release-rename.js

# Locate artifacts (handles both --target and local build paths)
MAC_TARGET="src-tauri/target/aarch64-apple-darwin/release/bundle"
if [ ! -d "$MAC_TARGET" ]; then
  MAC_TARGET="src-tauri/target/release/bundle"
fi

MAC_DMG="$MAC_TARGET/dmg/caipi_aarch64.dmg"
MAC_TGZ="$MAC_TARGET/macos/caipi.app.tar.gz"
MAC_SIG="$MAC_TARGET/macos/caipi.app.tar.gz.sig"

for f in "$MAC_DMG" "$MAC_TGZ" "$MAC_SIG"; do
  if [ ! -f "$f" ]; then
    echo "Error: Expected macOS artifact not found: $f"
    exit 1
  fi
done
echo "macOS artifacts ready."

# ---------------------------------------------------------------------------
# Step 2: Wait for Windows CI build
# ---------------------------------------------------------------------------

echo ""
echo "=== Waiting for Windows CI build ==="

RUN_ID=""
for i in $(seq 1 30); do
  RUN_ID=$(gh run list -w Release -L 10 --json databaseId,headSha \
    -q ".[] | select(.headSha == \"$COMMIT\") | .databaseId" | head -1)
  if [ -n "$RUN_ID" ]; then
    break
  fi
  echo "  Waiting for CI run to start... (attempt $i/30)"
  sleep 10
done

if [ -z "$RUN_ID" ]; then
  echo "Error: Could not find a Release workflow run for commit $COMMIT"
  echo "Make sure you've pushed to main."
  exit 1
fi

echo "Found CI run: $RUN_ID"
echo "Waiting for completion..."
gh run watch "$RUN_ID"

STATUS=$(gh run view "$RUN_ID" --json conclusion -q '.conclusion')
if [ "$STATUS" != "success" ]; then
  echo "Error: CI run failed with status: $STATUS"
  echo "Check: gh run view $RUN_ID --log"
  exit 1
fi

# ---------------------------------------------------------------------------
# Step 3: Download Windows artifacts
# ---------------------------------------------------------------------------

echo ""
echo "=== Downloading Windows artifacts ==="

WIN_DIR=$(mktemp -d)
gh run download "$RUN_ID" -n release-artifacts-windows -D "$WIN_DIR"

# Find artifacts (handles nested directory structure from upload)
WIN_EXE=$(find "$WIN_DIR" -name "caipi_x64.exe" | head -1)
WIN_SIG=$(find "$WIN_DIR" -name "caipi_x64.exe.sig" | head -1)

if [ -z "$WIN_EXE" ] || [ -z "$WIN_SIG" ]; then
  echo "Error: Windows artifacts not found in download"
  exit 1
fi
echo "Windows artifacts ready."

# ---------------------------------------------------------------------------
# Step 4: Generate latest.json
# ---------------------------------------------------------------------------

echo ""
echo "=== Generating latest.json ==="

MAC_SIG_CONTENT=$(cat "$MAC_SIG")
WIN_SIG_CONTENT=$(cat "$WIN_SIG")

RELEASE_DIR=$(mktemp -d)
cat > "$RELEASE_DIR/latest.json" << EOF
{
  "version": "$VERSION",
  "notes": "",
  "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "platforms": {
    "darwin-aarch64": {
      "signature": "$MAC_SIG_CONTENT",
      "url": "https://github.com/pietz/caipi/releases/download/v$VERSION/caipi.app.tar.gz"
    },
    "windows-x86_64": {
      "signature": "$WIN_SIG_CONTENT",
      "url": "https://github.com/pietz/caipi/releases/download/v$VERSION/caipi_x64.exe"
    }
  }
}
EOF

# Validate
jq -e '.platforms["darwin-aarch64"].signature and .platforms["windows-x86_64"].signature' \
  "$RELEASE_DIR/latest.json" >/dev/null
echo "latest.json generated."

# ---------------------------------------------------------------------------
# Step 5: Create GitHub release
# ---------------------------------------------------------------------------

echo ""
echo "=== Creating GitHub release v$VERSION ==="

gh release create "v$VERSION" \
  "$MAC_DMG" \
  "$MAC_TGZ" \
  "$MAC_SIG" \
  "$RELEASE_DIR/latest.json" \
  "$WIN_EXE" \
  "$WIN_SIG" \
  --repo pietz/caipi \
  --title "v$VERSION" \
  --notes "Release v$VERSION"

echo ""
echo "=== Release v$VERSION published! ==="
echo "https://github.com/pietz/caipi/releases/tag/v$VERSION"

# Cleanup temp dirs
rm -rf "$WIN_DIR" "$RELEASE_DIR"
