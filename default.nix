{
  craneLib,
  lib,
  stdenv,
  pkg-config,
  alsa-lib,
  apple-sdk,
  ...
}:
craneLib.buildPackage {
  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      (lib.fileset.fromSource (craneLib.cleanCargoSource ./.))
      ./assets
      ./configs/template.yaml
    ];
  };
  strictDeps = true;
  nativeBuildInputs = [pkg-config];
  buildInputs =
    (lib.optionals stdenv.hostPlatform.isLinux [
      alsa-lib
    ])
    ++ (lib.optionals stdenv.hostPlatform.isDarwin [
      apple-sdk
    ]);
}
