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
          gpuiLibs = import ./nix/gpui-libs.nix pkgs;
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
              git
              python3
            ];
            buildInputs = gpuiLibs.build;

            # gpui-fast opens Vulkan, GL and Wayland at run time.
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath gpuiLibs.run;
          };
        };
    };
}
