{
  description = "uni-sync-curve Rust project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
        };

        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
        ];

        buildInputs = with pkgs; [
          libusb1
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "uni-sync-curve";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          inherit nativeBuildInputs buildInputs;

          meta = with pkgs.lib; {
            description = "Set fan curves Lian-Li UNI fans on Linux";
            license = licenses.mit;
            maintainers = [ "tarunbod" ];
          };
        };

        nixosModules.default =
          { config, pkgs, ... }:
          {
            options.services.uni-sync-curve = {
              enable = pkgs.lib.mkEnableOption "uni-sync-curve service";

              configFile = pkgs.lib.mkOption {
                type = pkgs.lib.types.str;
                default = "/etc/uni-sync-curve.json";
                description = "Path to the uni-sync-curve configuration file";
              };
            };

            config =
              let
                cfg = config.services.uni-sync-curve;
              in
              pkgs.lib.mkIf cfg.enable {
                systemd.services.uni-sync-curve = {
                  description = "uni-sync-curve service";
                  after = [ "network.target" ];
                  wantedBy = [ "multi-user.target" ];
                  serviceConfig = {
                    ExecStart = "${self.packages.${system}.default}/bin/uni-sync-curve --config-file ${cfg.configFile}";
                    Restart = "on-failure";
                    User = "root";
                  };
                };
              };
          };
      }
    );
}
