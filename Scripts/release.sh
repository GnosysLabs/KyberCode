#!/usr/bin/env bash
# Cut a Kyber Code release from the local machine.
#
# Usage: ./Scripts/release.sh <version> "<notes>"
#   e.g. ./Scripts/release.sh 0.1.1 "Fixed blank General settings section."
#
# Builds the macOS (Apple Silicon) bundle, signs it with Developer ID, notarizes
# and staples it, signs the updater artifacts with the local key at
# ~/.tauri/kyber-updater.key, creates the GitHub release, and uploads the
# bundle + latest.json updater feed.
#
# Windows artifacts are added afterwards by ./Scripts/build-windows.sh, which
# merges the windows-x86_64 entry into latest.json on the same release.
#
# Apple code signing is a different key from the Tauri updater key. This script
# requires Developer ID Application in the login keychain and the notarytool
# profile AC_NOTARY (same setup as Noise).

set -euo pipefail

REPO="GnosysLabs/KyberCode"
KEY_FILE="$HOME/.tauri/kyber-updater.key"
WIN_KEY_FILE="$HOME/.tauri/kyber-updater.key" # also shipped to the Windows builder by build-windows.sh
APPLE_SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY:-Developer ID Application: Christopher McElvogue (4PDUNTF69S)}"
NOTARY_PROFILE="${APPLE_KEYCHAIN_PROFILE:-AC_NOTARY}"

VERSION="${1:?usage: release.sh <version> \"<notes>\"}"
NOTES="${2:-Kyber Code $VERSION}"

cd "$(dirname "$0")/.."

export PATH="$HOME/.cargo/bin:$PATH"

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
echo "==> Committing version bump (skipped when already committed)"
git add -A
if ! git diff --cached --quiet; then
  git commit -m "Release $VERSION" --quiet
  git push origin HEAD
fi

echo "==> Checking Apple signing + notarization"
export APPLE_SIGNING_IDENTITY
export APPLE_TEAM_ID="${APPLE_TEAM_ID:-4PDUNTF69S}"
if ! security find-identity -v -p codesigning | grep -F "$APPLE_SIGNING_IDENTITY" >/dev/null; then
  echo "error: missing signing identity: $APPLE_SIGNING_IDENTITY" >&2
  security find-identity -v -p codesigning >&2
  exit 1
fi
xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" --output-format json >/dev/null

echo "==> Building macOS bundle (aarch64)"
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_FILE")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npx tauri build --target aarch64-apple-darwin --bundles app

BUNDLE_DIR="src-tauri/target/aarch64-apple-darwin/release/bundle"
UPDATER_DIR="$BUNDLE_DIR/macos"
APP="$(find "$UPDATER_DIR" -maxdepth 1 -type d -name '*.app' -print -quit)"
if [ -z "$APP" ] || [ ! -d "$APP" ]; then
  echo "error: signed .app missing from $UPDATER_DIR" >&2
  ls "$UPDATER_DIR" >&2
  exit 1
fi

echo "==> Notarizing $(basename "$APP")"
NOTARY_ZIP="$(mktemp -t kyber-notarize).zip"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$NOTARY_ZIP"
xcrun notarytool submit "$NOTARY_ZIP" --keychain-profile "$NOTARY_PROFILE" --wait
rm -f "$NOTARY_ZIP"
xcrun stapler staple "$APP"
xcrun stapler validate "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
spctl --assess --type execute --verbose=4 "$APP"

echo "==> Packing updater + download zip from the stapled app"
# GitHub release assets cannot keep spaces — they become dots. Pack under
# those names so latest.json URLs match the files people actually download.
APP_TAR="$UPDATER_DIR/Kyber.Code.app.tar.gz"
HUMAN_ZIP="$UPDATER_DIR/Kyber.Code.zip"
COPYFILE_DISABLE=1 tar -C "$UPDATER_DIR" -czf "$APP_TAR" "$(basename "$APP")"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$HUMAN_ZIP"
npx tauri signer sign "$APP_TAR"
APP_SIG="$APP_TAR.sig"
if [ ! -f "$APP_SIG" ]; then
  echo "error: updater signature missing at $APP_SIG" >&2
  exit 1
fi

echo "==> Creating GitHub release $TAG"
gh release create "$TAG" \
  --repo "$REPO" \
  --title "Kyber Code $VERSION" \
  --notes "$NOTES"

echo "==> Uploading macOS artifacts"
gh release upload "$TAG" "$APP_TAR" "$APP_SIG" "$HUMAN_ZIP" --repo "$REPO" --clobber

PUB_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SIG="$(cat "$APP_SIG")"
APP_ASSET="$(basename "$APP_TAR")"
APP_URL_ASSET="$(python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))' "$APP_ASSET")"
URL="https://github.com/$REPO/releases/download/$TAG/$APP_URL_ASSET"

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
