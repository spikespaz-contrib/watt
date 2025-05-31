{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";

  outputs = {
    self,
    nixpkgs,
    ...
  } @ inputs: let
    forAllSystems = nixpkgs.lib.genAttrs ["x86_64-linux" "aarch64-linux"];
    pkgsForEach = forAllSystems (system:
      import nixpkgs {
        localSystem.system = system;
      });
  in {
    overlays = {
      superfreq = final: _: {
        superfreq = final.callPackage ./nix/package.nix {};
      };
      default = self.overlays.superfreq;
    };

    packages = forAllSystems (system: {
      superfreq = pkgsForEach.${system}.callPackage ./nix/package.nix {};
      default = self.packages.${system}.superfreq;
    });

    devShells = forAllSystems (system: {
      default = pkgsForEach.${system}.callPackage ./nix/shell.nix {};
    });

    nixosModules = {
      superfreq = import ./nix/module.nix inputs;
      default = self.nixosModules.superfreq;
    };

    formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.alejandra);
  };
}
