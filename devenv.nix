{
  pkgs,
  lib,
  ...
}: {
  packages =
    [
      pkgs.git
      pkgs.protobuf
      pkgs.grpcurl
    ]
    ++ lib.optionals pkgs.stdenv.isDarwin [
      pkgs.darwin.apple_sdk.frameworks.Security
      pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
    ];

  languages = {
    nix.enable = true;
    rust = {
      channel = "nightly";
      enable = true;
      components = ["rustc" "cargo" "clippy" "rustfmt" "rust-analyzer"];
    };
    python = {
      enable = true;
      venv.enable = true;
    };
  };

  services = {
    postgres.enable = true;
  };

  pre-commit.hooks = {
    clippy.enable = true;
    rustfmt.enable = true;
    alejandra.enable = true;
  };
}
