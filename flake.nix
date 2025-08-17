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
    flake-parts.lib.mkFlake {inherit inputs;} ({withSystem, ...}: {
      systems = ["x86_64-linux" "aarch64-darwin"];
      perSystem = {
        pkgs,
        system,
        ...
      }: let
        latestStableRust = pkgs: pkgs.rust-bin.stable.latest;
        waylandPath = pkgs:
          with pkgs;
            lib.makeLibraryPath [
              wayland
              libGL
              libxkbcommon
            ];
      in rec {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [(import inputs.rust-overlay)];
        };

        packages = with pkgs; let
          craneLib =
            (inputs.crane.mkLib pkgs).overrideToolchain (p:
              (latestStableRust p).minimal);
          commonArgs = {
            src = ./.;
            strictDeps = true;
            nativeBuildInputs = [pkg-config];
            buildInputs =
              (lib.optionals stdenv.isLinux [
                alsa-lib
              ])
              ++ (lib.optionals stdenv.isDarwin [
                darwin.apple_sdk.frameworks.CoreMIDI
              ]);
          };
          adaptuner-bin = craneLib.buildPackage (
            commonArgs // {cargoArtifacts = craneLib.buildDepsOnly commonArgs;}
          );
        in ({inherit adaptuner-bin;}
          // lib.optionalAttrs stdenv.isLinux {
            adaptuner-wayland = stdenv.mkDerivation {
              inherit (adaptuner-bin) version pname;
              phases = ["installPhase"];
              installPhase = ''
                mkdir -p $out/bin
                patchelf \
                  --add-rpath "${waylandPath pkgs}" \
                  --output $out/bin/adaptuner \
                  ${adaptuner-bin}/bin/adaptuner

                mkdir -p $out/share/applications
                cp ${./assets/adaptuner.desktop} $out/share/applications/adaptuner.desktop

                mkdir -p $out/share/icons/hicolor/scalable/apps
                cp ${./assets/icon/aplus.svg} $out/share/icons/hicolor/scalable/apps/adaptuner.svg
              '';
            };
          });

        devShells = pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
          wayland = pkgs.mkShell {
            inputsFrom = [packages.adaptuner-bin];
            packages = with pkgs;
              [
                fluidsynth
                vmpk

                # dev-y
                (latestStableRust pkgs).rust-analyzer
                (latestStableRust pkgs).rustfmt
                bacon
              ]
              ++ lib.optionals stdenv.isLinux [alsa-utils];

            LD_LIBRARY_PATH = waylandPath pkgs;
          };
        };
      };
    });
}
