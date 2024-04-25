{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ { self, ... }:
    (inputs.flake-utils.lib.eachDefaultSystem (system:
      let

        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [ inputs.rust-overlay.overlays.default ];
        };

        rust = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain).override {
          extensions = [
            "rust-src"
          ];
        };
        # rustfmt from rust-nightly used for advanced options in rustfmt
        rustfmt-nightly = pkgs.rust-bin.nightly.latest.rustfmt;

        shellPkgs = [
          rustfmt-nightly
          rust
        ] ++ (with pkgs; [
          ets
          just
          nixpkgs-fmt
          openssl
          perl
          pkg-config
          present-cli
        ]);

      in
      rec {

        devShells = {
          default = pkgs.mkShell {
            buildInputs = shellPkgs;
          };
        };

      }));
}
