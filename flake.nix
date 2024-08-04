{
  description = "A basic flake with a shell";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.naersk.url = "github:nix-community/naersk";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      naersk,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        rust = pkgs.rust-bin.stable.latest.default;
        naersk' = pkgs.callPackage naersk {
          cargo = rust;
          rustc = rust;
        };
      in
      {
        devShell = pkgs.mkShell {
          nativeBuildInputs =
            [ rust ]
            ++ (with pkgs; [
              diesel-cli
              calibre
            ]);
          RUST_PATH = "${rust}";
          RUST_DOC_PATH = "${rust}/share/doc/rust/html/std/index.html";

          shellHook = ''
            export PROJECT_ROOT=$(realpath .)
            export BOUQUINEUR_CONFIG=$PROJECT_ROOT/config.toml
            export DATABASE_URL=postgres://@/bouquineur
          '';
        };

        packages.default = naersk'.buildPackage {
          src = ./.;
          buildInputs = with pkgs; [ postgresql.lib ];
          meta.mainProgram = "bouquineur";
        };
      }
    );
}
