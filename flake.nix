{
  description = "adaptuner";

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
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
      overlays = [(import rust-overlay)];
    };
    rust-bin = pkgs.rust-bin.nightly.latest;
    rustPlatform = pkgs.makeRustPlatform (with rust-bin; {
      cargo = minimal;
      rustc = minimal;
    });
    adaptuner = rustPlatform.buildRustPackage {
      pname = "adaptuner";
      version = "0.0.0";
      src = ./.;
      cargoLock.lockFile = ./Cargo.lock;
      nativeBuildInputs = with pkgs; [pkg-config];
      buildInputs = with pkgs; [alsa-lib];
    };
  in {
    packages.${system}.default = adaptuner;

    devShells.${system}.default = pkgs.mkShell {
      inputsFrom = [adaptuner];

      packages = with pkgs; [
        fluidsynth
        vmpk
        alsa-utils

        # dev-y
        rust-bin.rust-analyzer
        rust-bin.rustfmt
	bacon
        jq

        # tex
        texlive.combined.scheme-full
        latexrun
      ];

      LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath (with pkgs; [
        wayland
        libGL
        libxkbcommon
      ])}";
    };
  };
}
