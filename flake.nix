{
  description = "RimStack: Fast RimWorld mod manager";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{
      flake-parts,
      nixpkgs,
      rust-overlay,
      self,
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      perSystem =
        {
          config,
          pkgs,
          system,
          ...
        }:
        let
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          qtPrefix = pkgs.symlinkJoin {
            name = "rimstack-qt-prefix";
            paths = [
              pkgs.qt6.qtbase
              pkgs.qt6.qtdeclarative
            ];
          };
          qtConf = pkgs.writeText "qt.conf" ''
            [Paths]
            Prefix=${qtPrefix}
          '';
          qmake = pkgs.writeShellScript "qmake" ''exec ${pkgs.lib.getExe' pkgs.qt6.qtbase "qmake"} -qtconf ${qtConf} "$@"'';
        in
        {
          _module.args.pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          packages.rimstack = pkgs.callPackage ./nix/package.nix {
            src = self;
          };

          packages.default = config.packages.rimstack;

          devShells.default = pkgs.mkShell {
            name = "rimstack-dev";
            nativeBuildInputs = with pkgs; [
              rustToolchain
              cargo-audit
              just
              pkg-config
              qt6.wrapQtAppsHook
              git
              python3
            ];
            buildInputs = with pkgs; [
              qt6.qtbase
              qt6.qtdeclarative
            ];

            shellHook = ''
              export QMAKE=${qmake}
            '';
          };
        };
    };
}
