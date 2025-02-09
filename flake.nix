{
  description = "RP2040 Environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        formatter = nixpkgs.legacyPackages.${system}.nixfmt-rfc-style;
      in
      {
        formatter = formatter; # Format this file with `nix fmt flake.nix`
        devShells.default = pkgs.mkShell {
          pure = true;

          buildInputs = with pkgs; [
            (rust-bin.selectLatestNightlyWith (
              toolchain:
              toolchain.default.override {
                targets = [ "thumbv6m-none-eabi" ];
                extensions = [ "rust-src" ];
              }
            ))
            probe-rs
            flip-link
          ];

          shellHook = ''
            echo "Welcome to the RP2040 Environment!"
            echo "Rust version: $(rustc --version)"
            echo "Probe-rs version: $(probe-rs --version)"
          '';
        };
      }
    );
}
