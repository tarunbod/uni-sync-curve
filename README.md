# uni-sync-curve

A CPU temperature-based fan control service for Lian Li PC case fans, for
Linux.

uni-sync-curve borrows code and is inspired by
[uni-sync](https://github.com/EightB1ts/uni-sync). While uni-sync allows
controlling fan speeds, it only updates the speed once when run. uni-sync-curve
continuously monitors CPU temperature and adjusts fan speeds based on
user-defined curves.

## Supported Devices

All devices supported by uni-sync:
- LianLi-UNI SL (PID: 7750, a100)
- LianLi-UNI AL (PID: a101)
- LianLi-UNI SL-Infinity (PID: a102)
- LianLi-UNI SL v2 (PID: a103, a105)
- LianLi-UNI AL v2 (PID: a104)

## Installation

If you use NixOS, you can install uni-sync-curve with the following config in
your flake:

```nix
{
  inputs.uni-sync-curve = {
    url = "github:tarunbod/uni-sync-curve";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, uni-sync-curve, ... }: {
    nixosConfigurations.your-machine = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        # ...

        uni-sync-curve.nixosModules.default

        {
            services.uni-sync-curve = {
              enable = true;
              configFile = "/etc/uni-sync-curve/uni-sync-curve.json"; # Optional
            };
        }

        # ...
      ];
      specialArgs = { inherit uni-sync-curve; };
    };
  };
}
```

If you don't use NixOS, you can build and run uni-sync-curve from source.

### Building

You will need:

- Rust (https://rustup.rs/)
- System dependencies:
  - Linux: `pkg-config`, `libusb-1.0-dev`

```bash
git clone <this-repo> cd uni-sync-curve cargo build --release
```

### Running

```bash
sudo ./target/release/uni-sync-curve [--config-file /path/to/config.json] [--debug]
```

Note: Root privileges are usually required to access USB devices.

## Configuration

The service will automatically create a configuration file based on detected
fan devices on first run at `/etc/uni-sync-curve/uni-sync-curve.json`, or the
specified path with `--config-file`.

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## License

This project follows the same license as the original uni-sync project (MIT).

## Acknowledgments

- [uni-sync](https://github.com/EightB1ts/uni-sync) - The original tool this
  project is built upon
- [EightB1ts](https://github.com/EightB1ts) - Creator of uni-sync
