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
scp -q Scripts/build-windows.ps1 "$HOST:C:/Users/cmcel/src/KyberCode/build-windows.ps1"
# Do not pipe the SSH session through tail/head — a full pipe buffer
# deadlocks npx tauri build.
ssh "$HOST" "powershell -ExecutionPolicy Bypass -File C:\\Users\\cmcel\\src\\KyberCode\\build-windows.ps1"

echo "==> Fetching Windows artifacts"
rm -rf .release-win && mkdir -p .release-win
ssh "$HOST" "powershell -NoProfile -Command \"
  Copy-Item -LiteralPath (Get-ChildItem 'C:\\Users\\cmcel\\src\\KyberCode\\src-tauri\\target\\release\\bundle\\nsis\\*$VERSION*-setup.exe' | Select-Object -First 1).FullName -Destination 'C:\\Users\\cmcel\\Kyber.Code_${VERSION}_x64-setup.exe' -Force
  \$sig = Get-ChildItem 'C:\\Users\\cmcel\\src\\KyberCode\\src-tauri\\target\\release\\bundle\\nsis\\*$VERSION*-setup.exe.sig' -ErrorAction SilentlyContinue | Select-Object -First 1
  if (\$sig) { Copy-Item -LiteralPath \$sig.FullName -Destination 'C:\\Users\\cmcel\\Kyber.Code_${VERSION}_x64-setup.exe.sig' -Force }
  Copy-Item -LiteralPath (Get-ChildItem 'C:\\Users\\cmcel\\src\\KyberCode\\src-tauri\\target\\release\\bundle\\msi\\*$VERSION*.msi' | Select-Object -First 1).FullName -Destination 'C:\\Users\\cmcel\\Kyber.Code_${VERSION}_x64_en-US.msi' -Force
\""
scp -q "$HOST:C:/Users/cmcel/Kyber.Code_${VERSION}_x64-setup.exe" .release-win/
scp -q "$HOST:C:/Users/cmcel/Kyber.Code_${VERSION}_x64-setup.exe.sig" .release-win/ || true
scp -q "$HOST:C:/Users/cmcel/Kyber.Code_${VERSION}_x64_en-US.msi" .release-win/ || true
ls .release-win/

SETUP_EXE=$(find .release-win -name "*-setup.exe" | head -1)
if [ -z "$SETUP_EXE" ]; then
  echo "error: expected NSIS setup exe" >&2
  ssh "$HOST" "dir C:\\Users\\cmcel\\src\\KyberCode\\src-tauri\\target\\release\\bundle\\nsis" || true
  exit 1
fi

SETUP_SIG="${SETUP_EXE}.sig"
if [ ! -f "$SETUP_SIG" ]; then
  echo "==> Signing Windows updater artifact"
  export TAURI_SIGNING_PRIVATE_KEY
  TAURI_SIGNING_PRIVATE_KEY="$(cat "$WIN_KEY_FILE")"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
  npx tauri signer sign "$SETUP_EXE"
fi
if [ ! -f "$SETUP_SIG" ]; then
  echo "error: updater signature missing at $SETUP_SIG" >&2
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

echo "==> Windows build for $VERSION attached to $TAG."
