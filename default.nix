{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "kotofetch";
  version = "0.2.21";

  src = pkgs.fetchFromGitHub {
    owner = "hxpe-dev";
    repo = "kotofetch";
    rev = "v${version}";
    sha256 = "sha256-mll98MC/kB3BisC9teohlBDW6jgaOwMJRqsoGLjsSpA=";
  };

  cargoHash = "sha256-nqTEAe7ODBg5SFxVdWW9AckT/y2YKZTAmlBtGqZ0ysE=";

  meta = with pkgs.lib; {
    description = "Minimalist fetch tool for Japanese quotes (written in Rust)";
    homepage = "https://github.com/hxpe-dev/kotofetch";
    license = licenses.mit;
    platforms = platforms.unix;
  };
}