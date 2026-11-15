{
  git,
  lib,
  makeWrapper,
  pkg-config,
  pkgs,
  rustPlatform,
  src,
}:
let
  gpuiLibs = import ./gpui-libs.nix pkgs;
in
rustPlatform.buildRustPackage {
  pname = "rimstack";
  version = "${(lib.importTOML "${src}/Cargo.toml").workspace.package.version}-unstable-${
    lib.substring 0 8 src.lastModifiedDate
  }";

  inherit src;

  cargoLock.lockFile = "${src}/Cargo.lock";

  nativeBuildInputs = [
    makeWrapper
    pkg-config
  ];

  buildInputs = gpuiLibs.build;

  # Upstream's default branch declares a rust-version newer than nixpkgs' rustc.
  cargoBuildFlags = [
    "--ignore-rust-version"
    "--package"
    "rimstack-app"
  ];
  # The tests of the app crate need a display; core has the logic.
  cargoTestFlags = [
    "--ignore-rust-version"
    "--package"
    "rimstack-core"
  ];

  nativeCheckInputs = [ git ];
  checkFlags = [
    # Copies /bin/sleep, absent in the sandbox.
    "--skip=sources::maintenance::tests::a_game_started_now_is_seen_running_and_not_once_it_ended"
  ];

  postInstall = ''
    install -Dm644 crates/app/assets/rimstack.desktop -t $out/share/applications
    install -Dm644 crates/app/assets/rimstack.svg -t $out/share/icons/hicolor/scalable/apps
  '';

  # gpui-fast opens Vulkan, GL and Wayland at run time; git-sourced mods need Git.
  postFixup = ''
    wrapProgram $out/bin/rimstack \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath gpuiLibs.run} \
      --prefix PATH : ${lib.makeBinPath [ git ]}
  '';

  meta = {
    description = "Fast RimWorld mod manager";
    homepage = "https://github.com/TheForgotten69/RimStack";
    license = lib.licenses.gpl3Plus;
    mainProgram = "rimstack";
  };
}
