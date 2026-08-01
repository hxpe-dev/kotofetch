{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage rec {
  pname = "kotofetch";
  version = "0.2.23";

  src = pkgs.fetchFromGitHub {
    owner = "hxpe-dev";
    repo = "kotofetch";
    rev = "v${version}";
    sha256 = "sha256-mW0oMrmDUn8qsCCu870z6zS3szGVyCU4wnSytH6DkUE=";
  };

  cargoHash = "sha256-g5SLS6PpcRNm1zcHkX8pvqk7s2yQ+zUAjp4CSCq/U/o=";

  meta = with pkgs.lib; {
    description = "Minimalist fetch tool for Japanese quotes (written in Rust)";
    homepage = "https://github.com/hxpe-dev/kotofetch";
    license = licenses.mit;
    platforms = platforms.unix;
  };
}