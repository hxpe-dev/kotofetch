{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "kotofetch";
  version = "0.2.18";

  src = pkgs.fetchFromGitHub {
    owner = "hxpe-dev";
    repo = "kotofetch";
    rev = "v${version}";
    sha256 = "sha256-sU+GeZKr8Tpg52XzxLmuA3NyqA47wqANL4yReFJpI4M=";
  };

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  meta = with pkgs.lib; {
    description = "Minimalist fetch tool for Japanese quotes (written in Rust)";
    homepage = "https://github.com/hxpe-dev/kotofetch";
    license = licenses.mit;
    platforms = platforms.unix;
  };
}
