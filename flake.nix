{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
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

        craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rust;

        commonBuildInputs = [
          rust
        ] ++ (with pkgs; [
          openssl
        ]);

        commonNativeBuildInputs = with pkgs; [
          pkg-config
        ];

        shellPkgs = [
          # this needs to come first in list to override the default rustfmt
          rustfmt-nightly
        ] ++ (with pkgs; [
          ets
          gnumake
          just
          nixpkgs-fmt
          perl
          present-cli
        ])
        ++ commonBuildInputs
        ++ commonNativeBuildInputs;

        cargo-workspace = craneLib.buildPackage {
          pname = "cargo-workspace";
          src = craneLib.cleanCargoSource (craneLib.path ./.);
          strictDeps = true;
          buildInputs = commonBuildInputs;
          nativeBuildInputs = commonNativeBuildInputs;
          SWAGGER_UI_DOWNLOAD_URL = "file:${pkgs.fetchurl {
            url = "https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip";
            hash = "sha256-SBJE0IEgl7Efuu73n3HZQrFxYX+cn5UU5jrL4T5xzNw=";
          }}";
        };

        buildWorkspaceBinary = src:
          pkgs.stdenv.mkDerivation rec {
            inherit (craneLib.crateNameFromCargoToml { inherit src; })
              pname
              version
              ;
            phases = [ "installPhase" ];
            installPhase = ''
              mkdir -p $out/bin
              cp -r ${cargo-workspace}/bin/${pname} $out/bin/
            '';
          };

        cid-router = buildWorkspaceBinary ./cid-router;
        azure-blob-storage-crp = buildWorkspaceBinary ./external-crps/azure-blob-storage-crp;
        github-crp = buildWorkspaceBinary ./external-crps/github-crp;

        imageBase =
          pkgs.dockerTools.buildLayeredImage {
            name = "docker-service-image-base";
            # 1 layer holds customizations and packages are distributed among other layers
            # for maxLayers=2 all packages go in one layer
            maxLayers = 2;
            contents = with pkgs; [
              bashInteractive
              dockerTools.binSh
              dockerTools.usrBinEnv
              coreutils
              which
            ];
            config = {
              Env = [
                "USER=root"
                "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
              ];
            };
          };

        buildImage = binary-pkg:
          pkgs.dockerTools.buildLayeredImage {
            name = binary-pkg.pname;
            tag = "dev";
            fromImage = imageBase;
            maxLayers = 4; # base image uses 2 layers, this uses 2 layers
            contents = [
              binary-pkg
            ];
            config = {
              ExposedPorts = { "80/tcp" = { }; };
              EntryPoint = [
                "${binary-pkg}/bin/${binary-pkg.pname}"
              ];
            };
          };

        cid-router-image = buildImage cid-router;
        azure-blob-storage-crp-image = buildImage azure-blob-storage-crp;
        github-crp-image = buildImage github-crp;

      in
      rec {

        devShells = {
          default = pkgs.mkShell {
            buildInputs = shellPkgs;
            shellHook = ''
              export PATH="$(pwd)/_build/bin/:$PATH"
            '';
          };
        };

        packages = {
          inherit
            cid-router
            cid-router-image
            azure-blob-storage-crp
            azure-blob-storage-crp-image
            github-crp
            github-crp-image
            ;
        };

      }));
}
