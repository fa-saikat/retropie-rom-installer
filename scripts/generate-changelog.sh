#!/usr/bin/env sh
# Prepend a packaging/changelog entry for the version in Cargo.toml, built
# from commit subjects since the last tag. Bump Cargo.toml first; the script
# refuses to write a duplicate entry. Tags are fetched first, since CI
# creates them. It doesn't build, commit or tag.
#
# MAINTAINER_NAME / MAINTAINER_EMAIL override the git identity.

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
# Releases are tagged by CI, so the local clone may not have the latest tag
# yet. Without it the entry would repeat commits from earlier releases.
if ! git fetch --tags --quiet; then
    echo "error: couldn't fetch tags from the remote; the changelog would" >&2
    echo "       include commits that were already released." >&2
    exit 1
fi

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
