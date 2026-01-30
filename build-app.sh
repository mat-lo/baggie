#!/bin/bash
set -e

# Configuration
APP_NAME="Baggie"
BUNDLE_ID="com.baggie.app"
SIGN_IDENTITY="Developer ID Application: oio.studio ltd (5H3PSA638Y)"
NOTARIZE_PROFILE="baggie-notarize"

echo "Building release binary..."
cargo build --release

echo "Creating app bundle..."
rm -rf "target/${APP_NAME}.app"
mkdir -p "target/${APP_NAME}.app/Contents/MacOS"
mkdir -p "target/${APP_NAME}.app/Contents/Resources"

echo "Copying files..."
cp "target/release/baggie" "target/${APP_NAME}.app/Contents/MacOS/"
cp "Info.plist" "target/${APP_NAME}.app/Contents/"
cp "AppIcon.icns" "target/${APP_NAME}.app/Contents/Resources/"

echo "Signing app with identity: ${SIGN_IDENTITY}"
codesign --force --deep --options runtime --sign "${SIGN_IDENTITY}" "target/${APP_NAME}.app"

echo "Verifying signature..."
codesign --verify --verbose "target/${APP_NAME}.app"

echo ""
echo "Notarizing app..."
rm -f "target/${APP_NAME}.zip"
ditto -c -k --keepParent "target/${APP_NAME}.app" "target/${APP_NAME}.zip"
xcrun notarytool submit "target/${APP_NAME}.zip" --keychain-profile "${NOTARIZE_PROFILE}" --wait

echo "Stapling notarization ticket..."
xcrun stapler staple "target/${APP_NAME}.app"

rm "target/${APP_NAME}.zip"

echo ""
echo "Done! App bundle created at: target/${APP_NAME}.app"
echo "App is signed, notarized, and ready for distribution."
