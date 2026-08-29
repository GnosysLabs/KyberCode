#!/usr/bin/env bash
# Build the Windows (NSIS) bundle on the user's Windows PC over SSH and attach
# it to an existing release, merging the windows-x86_64 entry into latest.json.
#
# Usage: ./Scripts/build-windows.sh <version>
#   Run AFTER ./Scripts/release.sh <version> — this script expects the GitHub
#   release v<version> to exist with a latest.json already attached.

set -euo pipefail

REPO="GnosysLabs/KyberCode"
HOST="noise-windows"
WIN_KEY_FILE="$HOME/.tauri/kyber-updater.key"

VERSION="${1:?usage: build-windows.sh <version>}"
TAG="v$VERSION"

cd "$(dirname "$0")/.."

if [ ! -f "$WIN_KEY_FILE" ]; then
  echo "error: updater key missing at $WIN_KEY_FILE" >&2
  exit 1
fi
COMMIT="$(git rev-parse HEAD)"

echo "==> Preparing repo on $HOST"
ssh "$HOST" "
  if not exist C:\\Users\\cmcel\\src\\KyberCode (
    git clone https://github.com/$REPO.git C:\\Users\\cmcel\\src\\KyberCode
  )
  cd C:\\Users\\cmcel\\src\\KyberCode
  git fetch origin
  git checkout $COMMIT
"

echo "==> Syncing working tree"
git archive HEAD | ssh "$HOST" "cd C:\\Users\\cmcel\\src\\KyberCode && tar xf -"

echo "==> Building Windows bundle on $HOST"
# The signing key is referenced by path for this one build; it is not persisted
# on the Windows machine beyond the build itself.
scp -q "$WIN_KEY_FILE" "$HOST:C:/Users/cmcel/_kyber_signing.key"
# The build runs under PowerShell because the multiline signing key cannot be
# passed through cmd.exe environment variables.
scp -q Scripts/build-windows.ps1 "$HOST:C:/Users/cmcel/src/KyberCode/build-windows.ps1"
ssh "$HOST" "powershell -ExecutionPolicy Bypass -File C:\\Users\\cmcel\\src\\KyberCode\\build-windows.ps1" 2>&1 | tail -25
ssh "$HOST" "del C:\\Users\\cmcel\\_kyber_signing.key"

echo "==> Fetching Windows artifacts"
rm -rf .release-win && mkdir -p .release-win
scp -q "$HOST:C:/Users/cmcel/src/KyberCode/src-tauri/target/release/bundle/nsis/*-setup.exe" .release-win/ || true
scp -q "$HOST:C:/Users/cmcel/src/KyberCode/src-tauri/target/release/bundle/nsis/*-setup.exe.sig" .release-win/ || true
scp -q "$HOST:C:/Users/cmcel/src/KyberCode/src-tauri/target/release/bundle/msi/*.msi" .release-win/ || true
ls .release-win/

SETUP_EXE=$(find .release-win -name "*-setup.exe" | head -1)
SETUP_SIG=$(find .release-win -name "*-setup.exe.sig" | head -1)

if [ -z "$SETUP_EXE" ] || [ -z "$SETUP_SIG" ]; then
  echo "error: expected signed NSIS setup exe (updater artifact)" >&2
  ssh "$HOST" "dir C:\\Users\\cmcel\\src\\KyberCode\\src-tauri\\target\\release\\bundle\\nsis" || true
  exit 1
fi

echo "==> Uploading Windows artifacts"
MSI=$(find .release-win -name "*.msi" | head -1)
UPLOADS=("$SETUP_EXE" "$SETUP_SIG")
if [ -n "$MSI" ]; then
  UPLOADS+=("$MSI")
fi
gh release upload "$TAG" "${UPLOADS[@]}" --repo "$REPO" --clobber

echo "==> Merging windows-x86_64 into latest.json"
gh release download "$TAG" --repo "$REPO" --pattern latest.json --output .release-win/latest.json --clobber
python3 - <<EOF
import json, os, glob, urllib.parse

tag = "$TAG"
win_dir = ".release-win"

exe_path = glob.glob(os.path.join(win_dir, "*-setup.exe"))[0]
sig_path = exe_path + ".sig"

with open(os.path.join(win_dir, "latest.json")) as f:
    feed = json.load(f)

feed["platforms"]["windows-x86_64"] = {
    "signature": open(sig_path).read().strip(),
    "url": f"https://github.com/GnosysLabs/KyberCode/releases/download/{tag}/{urllib.parse.quote(os.path.basename(exe_path))}",
}

with open(os.path.join(win_dir, "latest.json"), "w") as f:
    json.dump(feed, f, indent=2)
    f.write("\n")
EOF
gh release upload "$TAG" .release-win/latest.json --repo "$REPO" --clobber

echo "==> Cleaning signing key off $HOST"
ssh "$HOST" "del C:\\Users\\cmcel\\_kyber_signing.key"

echo "==> Windows build for $VERSION attached to $TAG."
