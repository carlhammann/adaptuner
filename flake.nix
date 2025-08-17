{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-darwin"];
      perSystem = {
        pkgs,
        system,
        ...
      }: let
        rust-bin = pkgs.rust-bin.stable.latest;
      in rec {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [(import inputs.rust-overlay)];
        };

        packages = {
          default = let
            craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (_: rust-bin.minimal);
            commonArgs = {
              src = ./.;
              strictDeps = true;
              nativeBuildInputs = [pkgs.pkg-config];
              buildInputs =
                (pkgs.lib.optionals pkgs.stdenv.isLinux [pkgs.alsa-lib])
                ++ (pkgs.lib.optionals pkgs.stdenv.isDarwin [
                  pkgs.darwin.apple_sdk.frameworks.CoreMIDI
                ]);
            };
          in
            craneLib.buildPackage (
              commonArgs // {cargoArtifacts = craneLib.buildDepsOnly commonArgs;}
            );
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [packages.default];

          packages = with pkgs;
            [
              fluidsynth
              vmpk

              # dev-y
              rust-bin.rust-analyzer
              rust-bin.rustfmt
              bacon
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [alsa-utils];

          LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath (with pkgs;
            pkgs.lib.optionals stdenv.isLinux [
              wayland
              libGL
              libxkbcommon
            ])}";
        };
      };
    };
}
