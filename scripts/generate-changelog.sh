#!/usr/bin/env sh
# generate-changelog.sh
#
# Prepends a new entry to packaging/changelog, sourced from git commits,
# using the version currently set in Cargo.toml. This keeps Cargo.toml
# and packaging/changelog in sync by construction: the changelog entry
# is always generated FROM the version you just bumped, never typed by
# hand as a separate step.
#
# Usage:
#   ./scripts/generate-changelog.sh
#
# What it does:
#   1. Reads `version` from [package] in Cargo.toml.
#   2. Refuses to proceed if that version is already the top entry in
#      packaging/changelog (the "forgot to bump" case — bump Cargo.toml
#      first, then run this).
#   3. Collects commit subjects since the last git tag (or since the
#      last changelog entry's tag, or full history if there's no tag
#      yet) as the bullet list for the new entry.
#   4. Prepends a properly-formatted Debian changelog entry to
#      packaging/changelog (creating the file if it doesn't exist yet).
#
# Override maintainer identity with env vars if git config isn't set up
# the way you want it reflected in the changelog:
#   MAINTAINER_NAME="Fahim A Saikat" MAINTAINER_EMAIL="fahimabrar.saikat@gmail.com" \
#     ./scripts/generate-changelog.sh
#
# This script only edits packaging/changelog. It does not touch
# Cargo.toml, does not build anything, and does not create a git tag —
# run build-deb.sh separately afterward, and tag the release yourself
# (e.g. `git tag v$(VERSION)`) if that's part of your workflow.

set -e

CARGO_TOML="Cargo.toml"
CHANGELOG="packaging/changelog"

if [ ! -f "$CARGO_TOML" ]; then
    echo "error: $CARGO_TOML not found — run this from the project root" >&2
    exit 1
fi

# --- read package name + version from Cargo.toml -------------------------
PKG=$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$CARGO_TOML" | head -n1)
VERSION=$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$CARGO_TOML" | head -n1)

if [ -z "$PKG" ] || [ -z "$VERSION" ]; then
    echo "error: could not read name/version from $CARGO_TOML" >&2
    exit 1
fi

DEB_VERSION="${VERSION}-1"

# --- refuse to duplicate the current top entry ----------------------------
if [ -f "$CHANGELOG" ]; then
    TOP_VERSION=$(sed -n "1s/^${PKG} (\(.*\)).*/\1/p" "$CHANGELOG")
    if [ "$TOP_VERSION" = "$DEB_VERSION" ]; then
        echo "error: packaging/changelog already has an entry for ${DEB_VERSION}." >&2
        echo "       Bump the version in Cargo.toml first, then re-run this script." >&2
        exit 1
    fi
fi

# --- figure out the commit range for this entry ---------------------------
# Prefer commits since the last reachable git tag. If no tag exists yet
# (first release), fall back to the full history so the first changelog
# entry isn't empty.
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || true)

if [ -n "$LAST_TAG" ]; then
    RANGE="${LAST_TAG}..HEAD"
else
    RANGE="HEAD"
fi

COMMITS=$(git log --no-merges --pretty=format:'  * %s' "$RANGE" 2>/dev/null || true)

if [ -z "$COMMITS" ]; then
    COMMITS="  * No changes recorded since ${LAST_TAG:-project start}."
fi

# --- maintainer identity ---------------------------------------------------
NAME="${MAINTAINER_NAME:-$(git config user.name 2>/dev/null || echo "Unknown")}"
EMAIL="${MAINTAINER_EMAIL:-$(git config user.email 2>/dev/null || echo "unknown@example.com")}"

# --- build the new entry ----------------------------------------------------
DATE=$(date -R)

NEW_ENTRY=$(cat <<EOF
${PKG} (${DEB_VERSION}) unstable; urgency=low

${COMMITS}

 -- ${NAME} <${EMAIL}>  ${DATE}
EOF
)

# --- prepend to (or create) packaging/changelog -----------------------------
mkdir -p "$(dirname "$CHANGELOG")"

if [ -f "$CHANGELOG" ]; then
    {
        printf '%s\n\n' "$NEW_ENTRY"
        cat "$CHANGELOG"
    } > "${CHANGELOG}.tmp"
    mv "${CHANGELOG}.tmp" "$CHANGELOG"
else
    printf '%s\n' "$NEW_ENTRY" > "$CHANGELOG"
fi

echo "Added changelog entry for ${PKG} (${DEB_VERSION}) to ${CHANGELOG}:"
echo
echo "$NEW_ENTRY"
