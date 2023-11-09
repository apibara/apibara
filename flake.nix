{
  description = "Apibara development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-22.11";
    flake-utils = {
      url = "github:numtide/flake-utils";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane?rev=35110cccf28823320f4fd697fcafcb5038683982";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, crane, pre-commit-hooks, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [
          (import rust-overlay)
          (import ./nix/overlay.nix)
        ];

        pkgs = import nixpkgs {
          inherit system overlays;
        };

        crates = {
          starknet = {
            description = "The Starknet DNA server";
            path = ./starknet;
            ports = {
              "7171/tcp" = { };
            };
          };
          operator = {
            description = "The Apibara Kubernetese Operator";
            path = ./operator;
            ports = {
              "8118/tcp" = { };
            };
          };
          sink-console = {
            description = "Print stream data to the console";
            path = ./sinks/sink-console;
            volumes = {
              "/data" = { };
            };
            ports = {
              "8118/tcp" = { };
            };
          };
          sink-webhook = {
            description = "Integration to connect onchain data to HTTP endpoints";
            path = ./sinks/sink-webhook;
            volumes = {
              "/data" = { };
            };
            ports = {
              "8118/tcp" = { };
            };
          };
          sink-mongo = {
            description = "Integration to populate a MongoDB collection with onchain data";
            path = ./sinks/sink-mongo;
            volumes = {
              "/data" = { };
            };
            ports = {
              "8118/tcp" = { };
            };
          };
          sink-postgres = {
            description = "Integration to populate a PostgreSQL table with onchain data";
            path = ./sinks/sink-postgres;
            volumes = {
              "/data" = { };
            };
            ports = {
              "8118/tcp" = { };
            };
          };
          sink-parquet = {
            description = "Integration to generate a Parquet dataset from onchain data";
            path = ./sinks/sink-parquet;
            volumes = {
              "/data" = { };
            };
            ports = {
              "8118/tcp" = { };
            };
          };
          cli = {
            description = "Apibara CLI tool";
            path = ./cli;
            binaryName = "apibara";
            extraBinaries = [
              "sink-console"
              "sink-webhook"
              "sink-postgres"
              "sink-mongo"
              "sink-parquet"
            ];
            volumes = {
              "/data" = { };
            };
            ports = {
              "8118/tcp" = { };
            };
          };
        };

        builtCrates = pkgs.callPackage ./nix/crates.nix {
          inherit crane crates;
          pre-commit-hooks = pre-commit-hooks.lib.${system};
          workspaceDir = ./.;
        };

        ci = pkgs.callPackage ./nix/ci.nix {
          tests = builtCrates.packages.tests;
          binaries = builtCrates.binaries;
        };
      in
      {
        inherit (ci) pipeline;

        # format with `nix fmt`
        formatter = pkgs.nixpkgs-fmt;

        # checks. run with `nix flake check`.
        checks = builtCrates.checks;

        # development shells. start with `nix develop`.
        devShells = (builtCrates.shell // ci.shell);

        # all packages.
        # show them with `nix flake show`.
        # build with `nix build .#<name>`.
        packages = (builtCrates.packages // { });
      }
    );

  nixConfig = {
    extra-substituters = [ "https://apibara-public.cachix.org" ];
    extra-trusted-public-keys = [
      "apibara-public.cachix.org-1:FLOMNlARo9CcxtcLDblZlt2xhsu/pa/EddEH1cM3Vog="
    ];
  };
}
