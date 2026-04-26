#!/bin/bash
set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration — adjust paths if needed
# ---------------------------------------------------------------------------
KOTOFETCH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AUR_DIR="${AUR_DIR:-$HOME/Documents/Dev/kotofetch-aur}"
NIX_TEST_DIR="${NIX_TEST_DIR:-$HOME/nix-test}"

# ---------------------------------------------------------------------------
# Colors
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

step()    { echo -e "\n${BOLD}${BLUE}==> $*${NC}"; }
ok()      { echo -e "${GREEN}    [ok] $*${NC}"; }
warn()    { echo -e "${YELLOW}    [warn] $*${NC}"; }
die()     { echo -e "${RED}    [error] $*${NC}"; exit 1; }
confirm() {
    local prompt="$1"
    read -rp "$(echo -e "${YELLOW}    ${prompt} [y/N] ${NC}")" ans
    [[ "${ans,,}" == "y" ]]
}
pause() {
    read -rp "$(echo -e "${YELLOW}    $* — press Enter to continue...${NC}")"
}

# ---------------------------------------------------------------------------
# Rollback state — each flag is set just before the action it guards
# ---------------------------------------------------------------------------
CARGO_UPDATED=false
COMMITTED=false
PUSHED=false
TAGGED=false
TAG_PUSHED=false
AUR_COMMITTED=false
TAG=""
NEW_VERSION=""

rollback() {
    echo -e "\n${RED}${BOLD}Something went wrong. Rolling back completed steps...${NC}"

    if [[ "$AUR_COMMITTED" == true && -d "$AUR_DIR" ]]; then
        cd "$AUR_DIR"
        git reset --soft HEAD~1
        warn "Reverted AUR commit"
    fi

    cd "$KOTOFETCH_DIR"

    if [[ "$TAG_PUSHED" == true ]]; then
        if git push origin ":$TAG" 2>/dev/null; then
            warn "Removed remote tag $TAG"
        else
            warn "Could not remove remote tag $TAG — remove it manually: git push origin :$TAG"
        fi
    fi

    if [[ "$TAGGED" == true ]]; then
        git tag -d "$TAG" 2>/dev/null && warn "Removed local tag $TAG"
    fi

    if [[ "$PUSHED" == true ]]; then
        warn "The version bump commit was already pushed — remove it manually if needed:"
        warn "  git revert HEAD && git push"
    fi

    if [[ "$COMMITTED" == true && "$PUSHED" == false ]]; then
        git reset --soft HEAD~1
        warn "Reverted unpushed version bump commit"
    fi

    if [[ "$CARGO_UPDATED" == true ]]; then
        git checkout -- Cargo.toml Cargo.lock 2>/dev/null || git checkout -- Cargo.toml
        warn "Reverted Cargo.toml (and Cargo.lock if changed)"
    fi

    echo -e "${RED}    Rollback done. Fix the issue and re-run.${NC}"
}

trap 'exit_code=$?; [[ $exit_code -ne 0 ]] && rollback; exit $exit_code' EXIT

# ---------------------------------------------------------------------------
# 1. Version
# ---------------------------------------------------------------------------
step "Version"

NEW_VERSION="${1:-}"
if [[ -z "$NEW_VERSION" ]]; then
    CURRENT=$(grep '^version' "$KOTOFETCH_DIR/Cargo.toml" | head -1 | grep -oP '"\K[^"]+')
    echo "    Current version in Cargo.toml: $CURRENT"
    read -rp "    New version (e.g. 0.2.19): " NEW_VERSION
fi
NEW_VERSION="${NEW_VERSION#v}"
TAG="v$NEW_VERSION"
ok "Releasing $TAG"

# ---------------------------------------------------------------------------
# 2. Pre-flight checks
# ---------------------------------------------------------------------------
step "Pre-flight checks"

cd "$KOTOFETCH_DIR"

if ! grep -q "^## ${TAG}$" CHANGELOG.md; then
    die "No '## ${TAG}' section found in CHANGELOG.md — add release notes first."
fi
ok "CHANGELOG.md has section for $TAG"

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$BRANCH" != "main" ]]; then
    warn "Current branch is '$BRANCH', not 'main'."
    confirm "Continue anyway?" || exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
    warn "There are uncommitted changes."
    confirm "Continue anyway?" || exit 1
fi

if git rev-parse "$TAG" &>/dev/null; then
    die "Local tag $TAG already exists — delete it first: git tag -d $TAG"
fi
if git ls-remote --tags origin "$TAG" | grep -q "$TAG"; then
    die "Remote tag $TAG already exists — delete it first: git push origin :$TAG"
fi
ok "Tag $TAG is free"

# ---------------------------------------------------------------------------
# 3. Update Cargo.toml version
# ---------------------------------------------------------------------------
step "Updating Cargo.toml to $NEW_VERSION"

CURRENT_VERSION=$(grep '^version' "$KOTOFETCH_DIR/Cargo.toml" | head -1 | grep -oP '"\K[^"]+')
if [[ "$CURRENT_VERSION" == "$NEW_VERSION" ]]; then
    ok "Cargo.toml already at $NEW_VERSION"
else
    CARGO_UPDATED=true
    sed -i "0,/^version = \".*\"/s//version = \"$NEW_VERSION\"/" "$KOTOFETCH_DIR/Cargo.toml"
    ok "Bumped $CURRENT_VERSION -> $NEW_VERSION"
fi

# ---------------------------------------------------------------------------
# 4. Build check
# ---------------------------------------------------------------------------
step "Build check"
cargo build --release 2>&1 | tail -3
ok "Build passed"

# ---------------------------------------------------------------------------
# 5. Commit, push, tag
# ---------------------------------------------------------------------------
step "GitHub: commit, push, tag"

git add Cargo.toml Cargo.lock
if ! git diff --cached --quiet; then
    COMMITTED=true
    git commit -m "chore: bump to $TAG"
fi

PUSHED=true
git push

TAGGED=true
git tag -a "$TAG" -m "Release $TAG"

TAG_PUSHED=true
git push origin "$TAG"
ok "Pushed $TAG — GitHub CI will now build and publish the release"

# ---------------------------------------------------------------------------
# 6. Wait for GitHub release
# ---------------------------------------------------------------------------
step "Waiting for GitHub release"
echo "    The CI workflow is building the release binaries."
echo "    Check: https://github.com/hxpe-dev/kotofetch/actions"
pause "Once the release is published"

# ---------------------------------------------------------------------------
# 7. AUR update
# ---------------------------------------------------------------------------
step "AUR update"

if [[ ! -d "$AUR_DIR" ]]; then
    warn "AUR directory not found at $AUR_DIR"
    warn "Set AUR_DIR=/path/to/aur-repo and re-run, or skip this step."
    confirm "Skip AUR update?" && echo "    Skipping AUR." || die "Aborted."
else
    cd "$AUR_DIR"

    sed -i "s/^pkgver=.*/pkgver=$NEW_VERSION/" PKGBUILD
    sed -i "s/^pkgrel=.*/pkgrel=1/" PKGBUILD
    ok "Updated pkgver=$NEW_VERSION pkgrel=1 in PKGBUILD"

    echo "    Running makepkg -g to fetch and hash sources..."
    NEW_SUMS=$(makepkg -g 2>/dev/null)
    awk -v sums="$NEW_SUMS" '
        /^sha256sums=/ { found=1 }
        found && /\)/ { print sums; found=0; next }
        found { next }
        !found { print }
    ' PKGBUILD > PKGBUILD.tmp && mv PKGBUILD.tmp PKGBUILD
    ok "Updated sha256sums in PKGBUILD"

    echo "    Running makepkg -C to verify..."
    makepkg -C
    ok "PKGBUILD verified"

    makepkg --printsrcinfo > .SRCINFO
    git add PKGBUILD .SRCINFO
    AUR_COMMITTED=true
    git commit -m "Update to $NEW_VERSION"
    git push aur master
    ok "AUR updated"
fi

# ---------------------------------------------------------------------------
# 8. Nix update
# ---------------------------------------------------------------------------
step "Nix update"

mkdir -p "$NIX_TEST_DIR"
sed -i "s/version = \".*\"/version = \"$NEW_VERSION\"/" "$KOTOFETCH_DIR/default.nix"
cp "$KOTOFETCH_DIR/default.nix" "$NIX_TEST_DIR/default.nix"
cp "$KOTOFETCH_DIR/Cargo.lock" "$NIX_TEST_DIR/Cargo.lock"

echo "    Running nix-build to get the correct sha256 (first run will fail)..."
NEW_SHA=$(nix-build "$NIX_TEST_DIR/default.nix" 2>&1 | grep -oP '(?<=got:\s{1,20})sha256-\S+' | head -1 || true)

if [[ -z "$NEW_SHA" ]]; then
    warn "Could not auto-extract sha256 from nix-build output."
    echo "    Run: nix-build $NIX_TEST_DIR/default.nix"
    echo "    Copy the 'got:' sha256 value, then edit default.nix manually."
    pause "Once default.nix is updated"
else
    sed -i "s|sha256 = \"sha256-.*\"|sha256 = \"$NEW_SHA\"|" "$KOTOFETCH_DIR/default.nix"
    ok "Updated sha256 in default.nix: $NEW_SHA"

    cp "$KOTOFETCH_DIR/default.nix" "$NIX_TEST_DIR/default.nix"
    cp "$KOTOFETCH_DIR/Cargo.lock" "$NIX_TEST_DIR/Cargo.lock"
    echo "    Verifying nix build with new sha256..."
    nix-build "$NIX_TEST_DIR/default.nix"
    ok "Nix build verified"
fi

cd "$KOTOFETCH_DIR"
git add default.nix
git diff --cached --quiet || git commit -m "chore: nix bump to $TAG"
git push
ok "default.nix committed and pushed"

# ---------------------------------------------------------------------------
echo -e "\n${BOLD}${GREEN}Release $TAG complete.${NC}"
