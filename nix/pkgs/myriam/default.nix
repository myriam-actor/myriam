{ buildRustPackage, lib }:

let
  cargoProject = lib.importTOML ../../../Cargo.toml;
in

buildRustPackage {
  pname = cargoProject.package.name;
  version = cargoProject.package.version;

  src = lib.cleanSource ../../..;

  cargoHash = "sha256-kG8fWPd9C6We9LtF6dfBQ5KoM3CTgU3FhIZPq7xOX6g=";

  meta = with lib; {
    description = "Capability-secure distributed actors for Rust";
    homepage = "https://github.com/myriam-actor/myriam";
    license = licenses.asl20;
  };
}
