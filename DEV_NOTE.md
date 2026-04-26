# This is a dev note to myself, do not read it

## For each new update

Don't forget to write the [CHANGELOG](CHANGELOG.md).  
Don't forget to update the [Cargo.toml](Cargo.toml) file version.

In the github repo folder
```bash
git add .
git commit -m "commit message"
git push
git tag -a v0.2.1 -m "Release version 0.2.1"    # Example version
git push origin v0.2.1                          # Example version
```

In the AUR repo folder
```bash
nvim PKGBUILD # Once in here change the version to match the one in the github repo and put pkgrel back to 1
makepkg -g # Generate new sha256sums for all sources
# Copy the sha256sums output into PKGBUILD (replace old ones)
makepkg -C # Verify build + checksum validity
makepkg --printsrcinfo > .SRCINFO
git add PKGBUILD .SRCINFO
git commit -m "Update to 0.2.1"                 # Match version
git push aur master
```

For the [default.nix](default.nix) file
```bash
cp ~/Documents/Dev/kotofetch/default.nix ~/nix-test/default.nix
nix-build ~/nix-test/default.nix
# This will fail, copy the correct sha256
nvim ~/Documents/Dev/kotofetch/default.nix # Replace the outdated sha256 with the new one
git add default.nix
git commit -m "Nix bump to latest"
git push
```