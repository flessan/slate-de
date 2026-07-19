#!/bin/bash
# Slate-DE Release Preparation Script
# This script prepares the project for a new release

set -e

VERSION=${1:-"0.1.0"}
echo "=== Preparing Slate-DE Release v$VERSION ==="
echo ""

# Update version in Cargo.toml
echo "1. Updating version in Cargo.toml..."
sed -i "s/^version = .*/version = \"$VERSION\"/" Cargo.toml
echo "   ✓ Version updated to $VERSION"

# Update README if needed
echo ""
echo "2. README highlights:"
echo "   - One-click install: curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash"
echo "   - Manual installation instructions"
echo "   - Configuration documentation"

# Make all scripts executable
echo ""
echo "3. Ensuring scripts are executable..."
chmod +x scripts/install.sh
chmod +x scripts/uninstall.sh
chmod +x scripts/test-install.sh
chmod +x install.sh
echo "   ✓ All scripts are executable"

# Create release notes template
echo ""
echo "4. Release checklist:"
echo "   □ Update CHANGELOG.md"
echo "   □ Test installer on fresh system"
echo "   □ Test on Ubuntu/Debian"
echo "   □ Test on Arch Linux"
echo "   □ Test on Fedora"
echo "   □ Verify uninstall works correctly"
echo "   □ Update documentation"
echo "   □ Create GitHub release"
echo "   □ Update AUR package (if applicable)"
echo "   □ Announce release"

# Git commands (optional)
echo ""
read -p "Create git tag v$VERSION? (y/N) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git add -A
    git commit -m "Release v$VERSION"
    git tag -a "v$VERSION" -m "Slate-DE v$VERSION"
    echo "   ✓ Git tag created"
    echo "   ℹ Push with: git push origin main --tags"
fi

echo ""
echo "=== Release Preparation Complete ==="
echo ""
echo "Next steps:"
echo "  1. Test the installer: curl -sSf http://localhost:8000/install.sh | bash"
echo "  2. Push changes: git push origin main --tags"
echo "  3. Create GitHub release with binaries"
