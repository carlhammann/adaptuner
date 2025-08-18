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

  outputs = inputs @ {flake-parts, ...}: let
    latestStableRust = pkgs: pkgs.rust-bin.stable.latest;
    waylandPath = pkgs:
      with pkgs;
        lib.makeLibraryPath [
          wayland
          libGL
          libxkbcommon
        ];
  in
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-darwin"];
      perSystem = {
        pkgs,
        system,
        ...
      }: rec {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [(import inputs.rust-overlay)];
        };

        packages = let
          adaptuner-bin = pkgs.callPackage ./. {
            craneLib =
              (inputs.crane.mkLib pkgs).overrideToolchain (p:
                (latestStableRust p).minimal);
          };
        in
          {
            inherit adaptuner-bin;
          }
          // (with pkgs;
            lib.optionalAttrs stdenv.isLinux {
              adaptuner-wayland = stdenv.mkDerivation {
                inherit (adaptuner-bin) version pname;
                phases = ["installPhase"];
                installPhase = ''
                  mkdir -p $out/bin
                  patchelf \
                    --add-rpath "${waylandPath pkgs}" \
                    --output $out/bin/adaptuner \
                    ${adaptuner-bin}/bin/adaptuner

                  mkdir $out/share
                  cp -r ${./share}/* $out/share
                '';
              };

              adaptuner-deb = stdenv.mkDerivation rec {
                inherit (adaptuner-bin) version pname;
                nativeBuildInputs = [dpkg];
                phases = ["installPhase"];
                # Who knows what the compiled binary actually uses? -- Let's find out!
                glibc-version =
                  runCommand "adaptuner-glibc-version" {
                    nativeBuildInputs = [gcc]; # gcc derivation has objdump
                  } ''
                    touch $out
                    objdump --dynamic-syms ${adaptuner-bin}/bin/adaptuner \
                     | grep GLIBC \
                     | sed 's/.*GLIBC_\([.0-9]*\).*/\1/g' \
                     | sort -Vu \
                     | tail -n1 \
                     | tr -d '\n' \
                     | tee $out
                  '';

                installPhase = let
                  controlFileContents = ''
                    Package: ${pname}
                    Version: ${version}
                    Section: misc
                    Priority: optional
                    Architecture: amd64
                    Depends: libasound2, libc6 (>= ${builtins.readFile "${glibc-version}"})
                    Maintainer: Carl Hammann <carl@carlhammann.com>
                    Description: adaptive MIDI tuner
                  '';
                in ''
                  debdir=${pname}_${version}_amd64
                  mkdir -p $debdir/usr/bin
                  patchelf \
                    --remove-rpath \
                    --set-interpreter /usr/lib64/ld-linux-x86-64.so.2 \
                    --output $debdir/usr/bin/adaptuner \
                    ${adaptuner-bin}/bin/adaptuner
                  strip $debdir/usr/bin/adaptuner

                  mkdir $debdir/usr/share
                  cp -r ${./share}/* $debdir/usr/share

                  mkdir $debdir/DEBIAN
                  echo '${controlFileContents}' > $debdir/DEBIAN/control

                  dpkg-deb --build $debdir

                  mkdir $out
                  mv *.deb $out/
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

                quickemu
              ]
              ++ lib.optionals stdenv.isLinux [alsa-utils];

            LD_LIBRARY_PATH = waylandPath pkgs;
          };
        };
      };
    };
}
