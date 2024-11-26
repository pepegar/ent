{
  pkgs,
  lib,
  ...
}: {
  env = {
    DATABASE_URL = "postgres://ent:ent_password@localhost:5432/ent";
  };

  packages =
    [
      pkgs.git
      pkgs.protobuf
      pkgs.grpcurl
      pkgs.sqlx-cli
    ]
    ++ lib.optionals pkgs.stdenv.isDarwin [
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
      venv.requirements = ''
        files-to-prompt==0.4
      '';
    };
  };

  services.postgres = {
    enable = true;
    package = pkgs.postgresql_15;
    initialDatabases = [{name = "ent";}];
    initialScript = ''
      DO $$
       BEGIN
         IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'ent') THEN
           CREATE USER ent WITH PASSWORD 'ent_password' SUPERUSER CREATEDB;
         END IF;
       END
       $$;

       GRANT ALL PRIVILEGES ON DATABASE ent TO ent;
       \c ent;

       -- Create schema if it doesn't exist
       CREATE SCHEMA IF NOT EXISTS public;

       -- Grant privileges
       ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO ent;
       GRANT ALL ON ALL TABLES IN SCHEMA public TO ent;
       GRANT ALL ON SCHEMA public TO ent;
    '';
    listen_addresses = "127.0.0.1";
    port = 5432;
    settings = {
      "timezone" = "UTC";
      "max_connections" = "100";
    };
  };

  pre-commit.hooks = {
    clippy.enable = true;
    rustfmt.enable = true;
    alejandra.enable = true;
  };
}
