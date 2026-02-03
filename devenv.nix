{ pkgs, inputs, ... }:

{
  dotenv.enable = true;

  cachix.pull = [ "wrangler" ];

  packages = [
    pkgs.cargo-release
    pkgs.cargo-watch
    pkgs.lld
    pkgs.worker-build
    inputs.wrangler.packages.${pkgs.stdenv.hostPlatform.system}.wrangler
    inputs.dagger.packages.${pkgs.stdenv.hostPlatform.system}.dagger
  ];

  languages = {
    rust = {
      enable = true;
      channel = "stable";
      targets = [ "wasm32-unknown-unknown" ];
    };

    deno = {
      enable = true;
    };
  };
}
