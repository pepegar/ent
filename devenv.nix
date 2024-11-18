{pkgs, ...}: {
  packages = [
    pkgs.git
    pkgs.protobuf
    pkgs.grpcurl
  ];

  languages = {
    nix.enable = true;
    rust = {
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
