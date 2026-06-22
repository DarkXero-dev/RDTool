#!/usr/bin/env bash
set -euo pipefail

CARGO_TOML="src-tauri/Cargo.toml"

# Parse flag
MODE="none"
for arg in "$@"; do
    case "$arg" in
        -r)  MODE="release" ;;
        -pr) MODE="prerelease" ;;
        *)   echo "Unknown flag: $arg  (use -r for release, -pr for pre-release)"; exit 1 ;;
    esac
done

# No-flag: just push whatever is committed, nothing else
if [[ "$MODE" == "none" ]]; then
    git push origin main
    echo ""
    echo "  Pushed"
    echo ""
    exit 0
fi

# Read current version
CURRENT=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')

# Bump patch
IFS='.' read -r MAJ MIN PAT <<< "$CURRENT"
NEW="$MAJ.$MIN.$((PAT + 1))"

echo ""
echo "  Version: $CURRENT  ->  $NEW"
[[ "$MODE" == "release" ]]    && echo "  Mode:    release  (tag: v$NEW)"
[[ "$MODE" == "prerelease" ]] && echo "  Mode:    pre-release  (tag: v$NEW-pre)"
echo ""
read -rp "  Commit message: " MSG
[[ -z "$MSG" ]] && { echo "Aborted: no message given."; exit 1; }

# Bump version in Cargo.toml
sed -i "s/^version = \"$CURRENT\"/version = \"$NEW\"/" "$CARGO_TOML"

# Update PKGBUILD pkgver
if [[ "$MODE" == "release" ]]; then
    sed -i "s/^pkgver=.*/pkgver=$NEW/" PKGBUILD
elif [[ "$MODE" == "prerelease" ]]; then
    sed -i "s/^pkgver=.*/pkgver=$NEW.pre/" PKGBUILD
fi

# Stage all modified tracked files + any already-staged new files
git add -u

git commit -m "$MSG"

git push origin main

if [[ "$MODE" == "release" ]]; then
    TAG="v$NEW"
else
    TAG="v$NEW-pre"
fi

# Delete all remote tags except the one we're about to push
# grep returns 1 on no match; use || true to avoid pipefail exit
REMOTE_TAGS=$(git ls-remote --tags origin | awk '{print $2}' | sed 's|refs/tags/||' || true)
if [[ -n "$REMOTE_TAGS" ]]; then
    OLD_REMOTE=$(echo "$REMOTE_TAGS" | grep -v "^${TAG}$" || true)
    if [[ -n "$OLD_REMOTE" ]]; then
        echo "$OLD_REMOTE" | xargs git push origin --delete
    fi
fi

# Remove any existing local tag with same name, create fresh
git tag -d "$TAG" 2>/dev/null || true
git tag "$TAG"
git push origin "$TAG"

echo ""
echo "  Pushed $TAG  (v$NEW)"
echo ""
