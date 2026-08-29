#!/usr/bin/env bash
# Cut a KyberCode release from the local machine.
#
# Usage: ./Scripts/release.sh <version> "<notes>"
#   e.g. ./Scripts/release.sh 0.1.1 "Fixed blank General settings section."
#
# Builds the macOS (Apple Silicon) bundle, signs the updater artifacts with the
# local key at ~/.tauri/kyber-updater.key, creates the GitHub release, and
# uploads the bundle + latest.json updater feed.
#
# Windows artifacts are added afterwards by ./Scripts/build-windows.sh, which
# merges the windows-x86_64 entry into latest.json on the same release.

set -euo pipefail

REPO="GnosysLabs/KyberCode"
KEY_FILE="$HOME/.tauri/kyber-updater.key"
WIN_KEY_FILE="$HOME/.tauri/kyber-updater.key" # also shipped to the Windows builder by build-windows.sh

VERSION="${1:?usage: release.sh <version> \"<notes>\"}"
NOTES="${2:-Kyber $VERSION}"

cd "$(dirname "$0")/.."

if [ ! -f "$KEY_FILE" ]; then
  echo "error: updater key missing at $KEY_FILE" >&2
  exit 1
fi
if [ -n "$(git status --porcelain)" ]; then
  echo "error: working tree is dirty — commit or stash first:" >&2
  git status --short >&2
  exit 1
fi

echo "==> Bumping version to $VERSION"
node -e "
  const fs = require('fs');
  for (const path of ['package.json']) {
    const pkg = JSON.parse(fs.readFileSync(path, 'utf8'));
    pkg.version = '$VERSION';
    fs.writeFileSync(path, JSON.stringify(pkg, null, 2) + '\n');
  }
"
python3 - <<EOF
import json, re

version = "$VERSION"

for path in ["src-tauri/tauri.conf.json", "src-tauri/Cargo.toml"]:
    with open(path) as f:
        text = f.read()
    if path.endswith(".json"):
        doc = json.loads(text)
        doc["version"] = version
        text = json.dumps(doc, indent=2) + "\n"
    else:
        text = re.sub(r'(?m)^version = ".*"$', f'version = "{version}"', text, count=1)
    with open(path, "w") as f:
        f.write(text)
EOF

TAG="v$VERSION"
echo "==> Committing version bump"
git add -A
git commit -m "Release $VERSION" --quiet
git push origin HEAD

echo "==> Building macOS bundle (aarch64)"
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_FILE")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npx tauri build --target aarch64-apple-darwin

BUNDLE_DIR="src-tauri/target/aarch64-apple-darwin/release/bundle"
UPDATER_DIR="$BUNDLE_DIR/macos"
APP_TAR="$UPDATER_DIR/Kyber.app.tar.gz"
APP_SIG="$APP_TAR.sig"
if [ ! -f "$APP_TAR" ]; then
  echo "error: updater artifact missing: $APP_TAR" >&2
  ls "$UPDATER_DIR" >&2
  exit 1
fi

echo "==> Creating GitHub release $TAG"
gh release create "$TAG" \
  --repo "$REPO" \
  --title "Kyber $VERSION" \
  --notes "$NOTES"

echo "==> Uploading macOS artifacts"
gh release upload "$TAG" "$APP_TAR" "$APP_SIG" --repo "$REPO" --clobber

PUB_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SIG="$(cat "$APP_SIG")"
URL="https://github.com/$REPO/releases/download/$TAG/Kyber.app.tar.gz"

cat > latest.json <<EOF
{
  "version": "$VERSION",
  "pub_date": "$PUB_DATE",
  "platforms": {
    "darwin-aarch64": {
      "signature": "$SIG",
      "url": "$URL"
    }
  }
}
EOF

echo "==> Uploading latest.json"
gh release upload "$TAG" latest.json --repo "$REPO" --clobber

echo "==> macOS release $VERSION published."
echo "    Next: ./Scripts/build-windows.sh $VERSION"
