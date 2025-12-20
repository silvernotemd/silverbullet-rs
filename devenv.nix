{ pkgs, inputs, ... }:

{
  dotenv.enable = true;

  cachix.pull = [ "wrangler" ];

  packages = [
    pkgs.cargo-release
    pkgs.cargo-watch
    pkgs.lld
    pkgs.worker-build
    inputs.wrangler.packages.${pkgs.stdenv.system}.wrangler
  ];

  languages.rust = {
    enable = true;
    channel = "stable";
  };
}
