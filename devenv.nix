{pkgs, ...}: {
  packages = [pkgs.git];
  languages.rust.enable = true;
  services.postgres.enable = true;
  pre-commit.hooks.clippy.enable = true;
  pre-commit.hooks.rustfmt.enable = true;
}
