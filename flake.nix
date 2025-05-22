{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    rust-overlay,
    ...
  }: let
    systems = ["x86_64-linux" "aarch64-darwin"];
    forAllSystems = f:
      nixpkgs.lib.genAttrs systems
      (system:
        f
        system
        (
          import nixpkgs {
            inherit system;
            overlays = [(import rust-overlay)];
          }
        ));
    rust-bin = forAllSystems (_: pkgs: pkgs.rust-bin.stable.latest);
    rustPlatform = forAllSystems (system: pkgs:
      pkgs.makeRustPlatform (with rust-bin.${system}; {
        cargo = minimal;
        rustc = minimal;
      }));
    adaptuner = forAllSystems (system: pkgs: let
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
    in
      rustPlatform.${system}.buildRustPackage {
        pname = cargoToml.package.name;
        version = cargoToml.package.version;
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        nativeBuildInputs = with pkgs; [pkg-config];
        buildInputs =
          (nixpkgs.lib.optionals pkgs.stdenv.isLinux [pkgs.alsa-lib])
          ++ (nixpkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.CoreMIDI
          ]);
      });
  in {
    packages = forAllSystems (system: _: {default = adaptuner.${system};});

    devShells = forAllSystems (system: pkgs: {
      default = pkgs.mkShell {
        inputsFrom = [adaptuner.${system}];

        packages = with pkgs;
          [
            fluidsynth
            vmpk

            # dev-y
            rust-bin.${system}.rust-analyzer
            rust-bin.${system}.rustfmt
            bacon
            jq

            # # tex
            # texlive.combined.scheme-full
            # latexrun
          ]
          ++ nixpkgs.lib.optionals pkgs.stdenv.isLinux [alsa-utils];

        LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath (with pkgs;
          nixpkgs.lib.optionals stdenv.isLinux [
            wayland
            libGL
            libxkbcommon
          ])}";
      };
    });
  };
}
