{
  description = "Capability-secure distributed actors for Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
  };

  outputs =
    { self, nixpkgs } @ inputs:
    let
      perSystem = systems: super:
        with nixpkgs.lib;
        let
          pkgsFor = system: import nixpkgs {
            overlays = attrValues self.overlays;
            inherit system;
          };
        in
        genAttrs systems (system: super (pkgsFor system));

      supportedSystems = [ "x86_64-linux" ];
    in
    {
      devShells.default = perSystem supportedSystems
        (pkgs: pkgs.callPackage ./nix/shell.nix {});

      legacyPackages = perSystem supportedSystems (pkgs: pkgs);

      overlays.default = import ./nix/pkgs;

      packages = perSystem supportedSystems (pkgs: {
        default = pkgs.myriam;
      });
    };
}
