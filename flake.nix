{
  description = "Cookiecrumbs development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Task runner
            just

            # Docker
            docker

            # Database tools
            postgresql
            dbmate

            # Redis tools
            redis

            # Python
            python3
            python3Packages.pip
            python3Packages.virtualenv

            # Java
            jdk
            gradle

            # Rust
            rustc
            cargo

            # Go
            go

            # Utilities
            curl
            jq
          ];

          shellHook = ''
            echo "Cookiecrumbs development environment"
            echo ""
            echo "Available commands:"
            echo "  just infra-up    - Start Postgres and Redis containers"
            echo "  just infra-down  - Stop containers"
            echo "  just migrate     - Apply database migrations"
            echo "  just run-django  - Run Django service"
            echo "  just run-java    - Run Java service"
            echo "  just run-rust    - Run Rust service"
            echo "  just run-go      - Run Go service"
            echo ""
          '';
        };
      }
    );
}
