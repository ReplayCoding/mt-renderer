{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, fenix }:
    let forAllSystems = nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed;
    in {
      # For `nix develop`:
      devShell = forAllSystems (system:
        let pkgs = (import nixpkgs) { inherit system; overlays = [ fenix.overlays.default ]; };
        in pkgs.mkShell {
          nativeBuildInputs = [
            (pkgs.fenix.complete.withComponents [
              "cargo"
              "clippy"
              "rustc"
              "rustfmt"
            ])
#             pkgs.rust-analyzer-nightly
          ];

          shellHook = ''
            export LD_LIBRARY_PATH=/run/opengl-driver/lib/:${
              pkgs.lib.makeLibraryPath (with pkgs; [
                libglvnd
                xorg.libXcursor
                xorg.libXi
                libxkbcommon
                xorg.libXrandr
                xorg.libX11
                vulkan-loader
                wayland
              ])
            }
          '';
        });
    };
}

