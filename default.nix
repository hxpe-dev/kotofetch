{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "kotofetch";
  version = "0.2.22";

  src = pkgs.fetchFromGitHub {
    owner = "hxpe-dev";
    repo = "kotofetch";
    rev = "v${version}";
    sha256 = "sha256-aY8HRKSHLQKjl4b7v5q3SeNMc+GJPnE2XVrEsl+nGR0=";
  };

  cargoHash = "sha256-r36x/I/RaIWFEoDYXf3edpLeqGvEyozhT4EuCTSEe/k=";

  meta = with pkgs.lib; {
    description = "Minimalist fetch tool for Japanese quotes (written in Rust)";
    homepage = "https://github.com/hxpe-dev/kotofetch";
    license = licenses.mit;
    platforms = platforms.unix;
  };
}