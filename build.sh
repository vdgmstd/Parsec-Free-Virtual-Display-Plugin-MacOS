#!/bin/bash
set -e

# Configuration file - single source of truth
CONFIG_FILE="PVDisplayPlugin.toml"

# Parse values from PVDisplayPlugin.toml
parse_toml() {
    local key=$1
    grep "^${key}" "${CONFIG_FILE}" 2>/dev/null | sed 's/.*= *"\{0,1\}\([^"]*\)"\{0,1\}/\1/' | head -1
}

# Parse array value (first element)
parse_toml_array() {
    local key=$1
    grep "^${key}" "${CONFIG_FILE}" 2>/dev/null | sed 's/.*\["\([^"]*\)".*/\1/' | head -1
}

# ============================================
# Read ALL config from PVDisplayPlugin.toml
# ============================================

# [application]
APP_DISPLAY_NAME=$(parse_toml "name")
VERSION=$(parse_toml "version")
OUT_DIR=$(parse_toml "out_dir")
ASSET_DIR=$(parse_toml "asset_dir")

# [bundle]
BUNDLE_ID=$(parse_toml "identifier")
PUBLISHER=$(parse_toml "publisher")
ICON_PATH=$(parse_toml_array "icon")
CATEGORY=$(parse_toml "category")
SHORT_DESC=$(parse_toml "short_description")
LONG_DESC=$(parse_toml "long_description")

# [bundle.macos]
MIN_MACOS=$(parse_toml "minimum_system_version")

# ============================================
# Defaults if not found in TOML
# ============================================
APP_NAME="PVDisplayPlugin"
APP_DISPLAY_NAME="${APP_DISPLAY_NAME:-Parsec Virtual Display Plugin}"
VERSION="${VERSION:-0.1.0}"
OUT_DIR="${OUT_DIR:-dist}"
ASSET_DIR="${ASSET_DIR:-assets}"
BUNDLE_ID="${BUNDLE_ID:-com.vdgmstd.parsec-vdisplay}"
PUBLISHER="${PUBLISHER:-vdgmstd}"
ICON_PATH="${ICON_PATH:-assets/icon.png}"
CATEGORY="${CATEGORY:-Utility}"
SHORT_DESC="${SHORT_DESC:-Virtual display manager for Parsec}"
LONG_DESC="${LONG_DESC:-Automatically manages virtual displays}"
MIN_MACOS="${MIN_MACOS:-11.0}"

# Map category to Apple format
case "${CATEGORY}" in
    "Utility"|"utility"|"utilities")
        APPLE_CATEGORY="public.app-category.utilities"
        ;;
    "Developer"|"developer")
        APPLE_CATEGORY="public.app-category.developer-tools"
        ;;
    "Productivity"|"productivity")
        APPLE_CATEGORY="public.app-category.productivity"
        ;;
    *)
        APPLE_CATEGORY="public.app-category.utilities"
        ;;
esac

# ============================================
# Directories (derived from TOML out_dir)
# ============================================
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RELEASES_DIR="${SCRIPT_DIR}/${OUT_DIR}"
APP_DIR="${RELEASES_DIR}/${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

echo "========================================"
echo "Building ${APP_NAME} v${VERSION}"
echo "========================================"
echo ""
echo "Configuration from ${CONFIG_FILE}:"
echo "  name:        ${APP_DISPLAY_NAME}"
echo "  version:     ${VERSION}"
echo "  out_dir:     ${OUT_DIR}"
echo "  asset_dir:   ${ASSET_DIR}"
echo "  identifier:  ${BUNDLE_ID}"
echo "  publisher:   ${PUBLISHER}"
echo "  icon:        ${ICON_PATH}"
echo "  category:    ${CATEGORY} -> ${APPLE_CATEGORY}"
echo "  min_macos:   ${MIN_MACOS}"
echo ""

# Build release binary for both architectures
echo "Compiling release binary (universal)..."

# Build for x86_64
echo "  -> Building x86_64..."
rustup target add x86_64-apple-darwin 2>/dev/null || true
cargo build --release --target x86_64-apple-darwin

# Build for arm64
echo "  -> Building arm64..."
rustup target add aarch64-apple-darwin 2>/dev/null || true
cargo build --release --target aarch64-apple-darwin

# Create universal binary
echo "Creating universal binary..."
mkdir -p "${RELEASES_DIR}"
lipo -create \
    "target/x86_64-apple-darwin/release/macos-parsec-free-vdisplay" \
    "target/aarch64-apple-darwin/release/macos-parsec-free-vdisplay" \
    -output "${RELEASES_DIR}/${APP_NAME}"

# Clean previous app bundle
rm -rf "${APP_DIR}"

# Create app bundle structure
echo "Creating app bundle..."
mkdir -p "${MACOS_DIR}"
mkdir -p "${RESOURCES_DIR}"

# Copy binary
cp "${RELEASES_DIR}/${APP_NAME}" "${MACOS_DIR}/${APP_NAME}"
chmod +x "${MACOS_DIR}/${APP_NAME}"

# Copy icon from path specified in TOML
if [ -f "${ICON_PATH}" ]; then
    cp "${ICON_PATH}" "${RESOURCES_DIR}/AppIcon.png"
    echo "  -> Copied icon from ${ICON_PATH}"
else
    echo "  -> Warning: Icon not found at ${ICON_PATH}"
fi

# Create Info.plist with ALL values from PVDisplayPlugin.toml
cat > "${CONTENTS_DIR}/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_DISPLAY_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>${MIN_MACOS}</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>LSApplicationCategoryType</key>
    <string>${APPLE_CATEGORY}</string>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © ${PUBLISHER}. ${SHORT_DESC}</string>
    <key>CFBundleGetInfoString</key>
    <string>${LONG_DESC}</string>
</dict>
</plist>
EOF

# Remove standalone binary (keep only .app)
rm -f "${RELEASES_DIR}/${APP_NAME}"

# Sign the app (ad-hoc signing for local use)
echo "Signing app bundle..."
codesign --force --deep --sign - "${APP_DIR}" 2>/dev/null || echo "Warning: codesign failed (may need Xcode)"

# ============================================
# Create DMG
# ============================================
echo "Creating DMG..."

DMG_NAME="${APP_NAME}-${VERSION}-universal"
DMG_PATH="${RELEASES_DIR}/${DMG_NAME}.dmg"
DMG_TEMP="${RELEASES_DIR}/dmg_temp"

# Cleanup previous
rm -rf "${DMG_TEMP}"
rm -f "${DMG_PATH}"

# Create temp folder with app and Applications symlink
mkdir -p "${DMG_TEMP}"
cp -r "${APP_DIR}" "${DMG_TEMP}/"
ln -s /Applications "${DMG_TEMP}/Applications"

# Create DMG
hdiutil create \
    -volname "${APP_NAME}" \
    -srcfolder "${DMG_TEMP}" \
    -ov \
    -format UDZO \
    "${DMG_PATH}"

# Cleanup temp
rm -rf "${DMG_TEMP}"

# Get sizes
APP_SIZE=$(du -sh "${APP_DIR}" | cut -f1)
DMG_SIZE=$(du -sh "${DMG_PATH}" | cut -f1)

echo ""
echo "========================================"
echo "Build complete!"
echo "========================================"
echo "  Output:  ${RELEASES_DIR}/"
echo "  App:     ${APP_DIR} (${APP_SIZE})"
echo "  DMG:     ${DMG_PATH} (${DMG_SIZE})"
echo "  Version: ${VERSION}"
echo ""
echo "To install:"
echo "  Open ${DMG_NAME}.dmg and drag to Applications"
