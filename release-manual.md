# Manual steps to create a new release (note to myself basically)

Run the docker container for kotofetch releases.

Then do:
```bash
export NEW_VERSION="0.2.19"
export TAG="v$NEW_VERSION"
export AUR_DIR="/home/builder/Documents/Dev/kotofetch-aur"
```

### Pre-flight verification & Cargo bump
```bash
# Verify CHANGELOG has your version entry
grep "^## ${TAG}$" CHANGELOG.md || echo "MISSING CHANGELOG ENTRY!"

# Test SSH access to GitHub and AUR
ssh -T git@github.com
ssh -T aur@aur.archlinux.org

# Bump version in Cargo.toml
sed -i "0,/^version = \".*\"/s//version = \"$NEW_VERSION\"/" Cargo.toml
``` 

### Build Check & Git Tagging (GitHub)
```bash
# Verify release build succeeds
cargo build --release

# Commit version bump and push tag
git add Cargo.toml Cargo.lock
git commit -m "chore: bump to $TAG"
git push
git tag -a "$TAG" -m "Release $TAG"
git push origin "$TAG"
```

Then wait for GitHub CI workflow to complete successfully.

### AUR Update (`makepkg`)

```bash
cd "$AUR_DIR"

# Update pkgver and pkgrel
sed -i "s/^pkgver=.*/pkgver=$NEW_VERSION/" PKGBUILD
sed -i "s/^pkgrel=.*/pkgrel=1/" PKGBUILD

# Calculate sha256 hash of published release tarball
TARBALL_URL="https://github.com/hxpe-dev/kotofetch/releases/download/${TAG}/kotofetch-${TAG}-x86_64-unknown-linux-gnu.tar.gz"
TARBALL_HASH=$(curl -sL "$TARBALL_URL" | sha256sum | awk '{print $1}')
echo "Tarball Hash: $TARBALL_HASH"

# Update sha256sums in PKGBUILD using Python
python3 - "$TARBALL_HASH" <<'PYEOF'
import sys, re
hash = sys.argv[1]
with open('PKGBUILD', 'r') as f:
    content = f.read()
new_block = "sha256sums=(\n  '{}'\n  'SKIP'\n  'SKIP'\n)".format(hash)
content = re.sub(r'sha256sums=\(.*?\)', new_block, content, flags=re.DOTALL)
with open('PKGBUILD', 'w') as f:
    f.write(content)
PYEOF

# Verify build with Arch tools and update .SRCINFO
makepkg -C
makepkg --printsrcinfo > .SRCINFO

# Commit and push to AUR
git add PKGBUILD .SRCINFO
git commit -m "Update to $NEW_VERSION"
git push origin master
```

### Nix Update (`nix-build`)

```bash
cd /app

# 1. Prepare temporary testing directory
mkdir -p ~/nix-test
cp default.nix ~/nix-test/default.nix
cd ~/nix-test

# 2. Update version and reset hashes in default.nix
sed -i "s/version = \".*\"/version = \"$NEW_VERSION\"/" default.nix
sed -i 's/sha256 = ".*"/sha256 = ""/' default.nix
sed -i 's/cargoHash = ".*"/cargoHash = ""/' default.nix

# 3. Get expected src hash (this command WILL fail and give you the correct hash)
nix-build default.nix

# 4. Copy the "got: sha256-..." hash from output and paste it into sha256 in default.nix, then run again:
sed -i 's|sha256 = ""|sha256 = "sha256-YOUR_SRC_HASH_HERE"|' default.nix
nix-build default.nix

# 5. Copy the new "got: sha256-..." cargo hash into cargoHash in default.nix, then verify final build:
sed -i 's|cargoHash = ""|cargoHash = "sha256-YOUR_CARGO_HASH_HERE"|' default.nix
nix-build default.nix

# 6. Copy updated default.nix back to main repo and commit
cp default.nix /app/default.nix
cd /app
git add default.nix
git commit -m "chore: nix bump to $TAG"
git push
```