#!/usr/bin/env bash
# Crea l'app macOS e il DMG di installazione: target/dmg/HostsManager-<versione>.dmg
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="HostsManager"
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
APP_PATH="target/release/bundle/osx/${APP_NAME}.app"
DMG_DIR="target/dmg"
DMG_PATH="${DMG_DIR}/${APP_NAME}-${VERSION}.dmg"
STAGING="${DMG_DIR}/staging"

if ! command -v cargo-bundle >/dev/null 2>&1; then
    echo "==> Installazione di cargo-bundle"
    cargo install cargo-bundle
fi

echo "==> Build dell'app ${APP_NAME} ${VERSION}"
cargo bundle --release

# Firma ad-hoc: senza un certificato Developer ID l'app non è notarizzata
echo "==> Firma ad-hoc"
codesign --force --deep --sign - "${APP_PATH}"

echo "==> Creazione del DMG"
rm -rf "${STAGING}"
mkdir -p "${STAGING}"
cp -R "${APP_PATH}" "${STAGING}/"
ln -s /Applications "${STAGING}/Applications"

hdiutil create \
    -volname "${APP_NAME}" \
    -srcfolder "${STAGING}" \
    -ov -format UDZO \
    "${DMG_PATH}" >/dev/null

rm -rf "${STAGING}"
echo "==> Creato ${DMG_PATH}"
