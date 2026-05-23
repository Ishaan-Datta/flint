{
  lib,
  stdenv,
  rustPlatform,
  makeBinaryWrapper,
  installShellFiles,
  versionCheckHook,
  jq,
  git,
  rev,
}:
let
  runtimeDeps = [
    git
    jq
  ];
  cargoToml = lib.importTOML ./Cargo.toml;
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "flint";
  version = "${cargoToml.workspace.package.version}";
  name = "${finalAttrs.pname}-${finalAttrs.version}-${rev}";

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.intersection (lib.fileset.fromSource (lib.sources.cleanSource ./.)) (
      lib.fileset.unions [
        ./.cargo
        ./.config
        ./flint
        ./xtask
        ./Cargo.toml
        ./Cargo.lock
      ]
    );
  };

  strictDeps = true;
  nativeBuildInputs = [
    installShellFiles
    makeBinaryWrapper
  ];

  cargoLock.lockFile = ./Cargo.lock;

  postInstall =
    lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
      # Run both shell completion and manpage generation tasks. Unlike the
      # fine-grained variants, the 'dist' command doesn't allow specifying the
      # path but that's fine, because we can simply install them from the implicit
      # output directories.
      $out/bin/xtask dist

      # The dist task above should've created
      #  1. Shell completions in comp/
      #  2. The flint manpage (flint.1) in man/
      # Let's install those.
      # The important thing to note here is that installShellCompletion cannot
      # actually load *all* shell completions we generate with 'xtask dist'.
      # Elvish, for example isn't supported. So we have to be very explicit
      # about what we're installing, or this will fail.
      installShellCompletion --cmd ${finalAttrs.meta.mainProgram} ./comp/*.{bash,fish,zsh,nu}
      installManPage ./man/flint.1
    ''
    + ''
      # Avoid populating PATH with an 'xtask' cmd
      rm $out/bin/xtask
    '';

  postFixup = ''
    wrapProgram $out/bin/flint \
      --prefix PATH : ${lib.makeBinPath runtimeDeps}
  '';

  nativeInstallCheckInputs = [ versionCheckHook ];
  doInstallCheck = true;
  versionCheckProgram = "${placeholder "out"}/bin/${finalAttrs.meta.mainProgram}";
  versionCheckProgramArg = "--version";

  nativeCheckInputs = runtimeDeps;

  useNextest = true;
  # Use nextest profile to filter tests that dont work in nix sandbox
  cargoTestFlags = [
    "--workspace"
    "--profile"
    "nix"
  ];
  checkFlags = [ ];

  meta = {
    description = "Flake Lock Lint";
    homepage = "https://github.com/Ishaan-Datta/flint";
    license = lib.licenses.gpl3;
    mainProgram = "flint";
  };
})
