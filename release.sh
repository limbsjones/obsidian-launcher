#!/bin/bash
# Create a GitHub release and upload the AppImage
# Usage: ./release.sh <version>

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 v1.0.0"
    exit 1
fi

VERSION="$1"
APPIMAGE="Obsidian-Launcher-x86_64.AppImage"

if [ ! -f "$APPIMAGE" ]; then
    echo "Error: $APPIMAGE not found"
    echo "Build it first with: appimagetool AppDir/ $APPIMAGE"
    exit 1
fi

# Try gh CLI
if command -v gh &>/dev/null; then
    echo "Creating release $VERSION with gh CLI..."
    gh release create "$VERSION" \
        --title "Obsidian Launcher $VERSION" \
        --notes "See README.md for details." \
        "$APPIMAGE"
    echo "✅ Release created!"
else
    echo "gh CLI not installed. Install it:"
    echo "  sudo pacman -S github-cli  # Arch"
    echo "  sudo apt install gh         # Debian/Ubuntu"
    echo ""
    echo "Then authenticate and run:"
    echo "  gh auth login"
    echo "  gh release create $VERSION --title 'Obsidian Launcher $VERSION' $APPIMAGE"
    echo ""
    echo "Or create the release manually at:"
    echo "  https://github.com/limbsjones/obsidian-launcher/releases/new"
fi
