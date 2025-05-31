{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";

  outputs = {
    self,
    nixpkgs,
    ...
  } @ inputs: let
    inherit (nixpkgs) lib;
    eachSystem = lib.genAttrs ["x86_64-linux" "aarch64-linux"];
    pkgsFor = eachSystem (system:
      import nixpkgs {
        localSystem.system = system;
        overlays = [self.overlays.default];
      });
  in {
    overlays = {
      superfreq = final: _: {
        superfreq = final.callPackage ./nix/package.nix {};
      };
      default = self.overlays.superfreq;
    };

    packages =
      lib.mapAttrs (system: pkgs: {
        inherit (pkgs) superfreq;
        default = self.packages.${system}.superfreq;
      })
      pkgsFor;

    devShells =
      lib.mapAttrs (system: pkgs: {
        default = pkgs.callPackage ./nix/shell.nix {};
      })
      pkgsFor;

    nixosModules = {
      superfreq = import ./nix/module.nix inputs;
      default = self.nixosModules.superfreq;
    };

    formatter = eachSystem (system: nixpkgs.legacyPackages.${system}.alejandra);
  };
}
