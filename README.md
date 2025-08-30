# uni-sync-curve

A temperature-based fan curve service built on top of [uni-sync](https://github.com/EightB1ts/uni-sync) for Lian Li fan controllers.

## Overview

uni-sync-curve extends the original uni-sync tool by providing continuous temperature monitoring and automatic fan speed adjustment based on configurable temperature curves. Instead of manually setting fan speeds, this service monitors your CPU temperature and adjusts fan speeds automatically according to your defined fan curves.

## Features

- **Continuous Temperature Monitoring**: Automatically reads CPU temperature at configurable intervals
- **Temperature-based Fan Curves**: Define custom fan curves with temperature-to-speed mappings
- **Multiple Device Support**: Supports all Lian Li devices compatible with uni-sync
- **Configurable Intervals**: Set how often temperature is checked and fan speeds are updated
- **Linear Interpolation**: Smooth fan speed transitions between curve points

## Supported Devices

All devices supported by uni-sync:
- LianLi-UNI SL (PID: 7750, a100)
- LianLi-UNI AL (PID: a101)
- LianLi-UNI SL-Infinity (PID: a102)
- LianLi-UNI SL v2 (PID: a103, a105)
- LianLi-UNI AL v2 (PID: a104)

## Installation

### Prerequisites

- Rust (https://rustup.rs/)
- System dependencies:
  - Linux: `pkg-config`, `libusb-1.0-dev`, `libudev-dev`
  - NixOS: `nix-shell -p pkg-config libusb1 udev`

### Building

```bash
git clone <this-repo>
cd uni-sync-curve

# On regular Linux systems:
sudo apt install pkg-config libusb-1.0-dev libudev-dev
cargo build --release

# On NixOS:
nix-shell -p pkg-config libusb1 udev --run "cargo build --release"
```

### Running

```bash
sudo ./target/release/uni-sync-curve
```

Note: Root privileges are required to access USB devices.

## Configuration

The service will automatically create a configuration file on first run:

- **Linux**: `/etc/uni-sync-curve/uni-sync-curve.json`
- **Windows**: `%PROGRAMDATA%\uni-sync-curve\uni-sync-curve.json`

### Example Configuration

```json
{
  "interval_seconds": 5,
  "fan_curves": [
    {
      "device_id": "VID:3314/PID:41216/SN:624314930/PATH:/dev/hidraw0",
      "channel": 0,
      "curve_points": [
        {
          "temperature_celsius": 30.0,
          "fan_speed_percent": 20
        },
        {
          "temperature_celsius": 50.0,
          "fan_speed_percent": 40
        },
        {
          "temperature_celsius": 70.0,
          "fan_speed_percent": 70
        },
        {
          "temperature_celsius": 85.0,
          "fan_speed_percent": 100
        }
      ]
    }
  ]
}
```

### Configuration Options

| Option | Description | Type | Example |
|--------|-------------|------|---------|
| `interval_seconds` | How often to check temperature and update fans | number | `5` |
| `fan_curves` | Array of fan curve configurations | array | See below |

#### Fan Curve Options

| Option | Description | Type | Example |
|--------|-------------|------|---------|
| `device_id` | Unique identifier for the device | string | `"VID:3314/PID:41216/SN:624314930/PATH:/dev/hidraw0"` |
| `channel` | Fan channel (0-3) | number | `0` |
| `curve_points` | Array of temperature-to-speed mappings | array | See below |

#### Curve Point Options

| Option | Description | Type | Range |
|--------|-------------|------|-------|
| `temperature_celsius` | Temperature in Celsius | number | Any positive value |
| `fan_speed_percent` | Fan speed percentage | number | 0-100 |

## How It Works

1. **Discovery**: On startup, the service discovers all connected Lian Li devices
2. **Temperature Monitoring**: Continuously monitors CPU temperature using system sensors
3. **Curve Calculation**: For each configured fan curve, calculates the appropriate fan speed based on current temperature:
   - If temperature is below the lowest curve point, uses minimum speed
   - If temperature is above the highest curve point, uses maximum speed
   - If temperature is between two points, linearly interpolates the speed
4. **Fan Control**: Applies the calculated fan speeds to the corresponding device channels

## Usage Examples

### Basic Setup

1. Run the service once to generate default configuration
2. Find your device IDs in the logs
3. Edit the configuration file to match your preferences
4. Run the service again

### Multiple Fans

You can configure multiple fan curves for different devices or channels:

```json
{
  "interval_seconds": 3,
  "fan_curves": [
    {
      "device_id": "VID:3314/PID:41216/SN:624314930/PATH:/dev/hidraw0",
      "channel": 0,
      "curve_points": [
        {"temperature_celsius": 30.0, "fan_speed_percent": 15},
        {"temperature_celsius": 80.0, "fan_speed_percent": 100}
      ]
    },
    {
      "device_id": "VID:3314/PID:41216/SN:624314930/PATH:/dev/hidraw0",
      "channel": 1,
      "curve_points": [
        {"temperature_celsius": 35.0, "fan_speed_percent": 25},
        {"temperature_celsius": 75.0, "fan_speed_percent": 90}
      ]
    }
  ]
}
```

## Troubleshooting

### Permission Issues
- Ensure you're running with root privileges
- Check that your user has access to USB devices

### No Devices Found
- Verify your Lian Li devices are connected
- Check that they're supported by the original uni-sync
- Try running the original uni-sync tool first

### Temperature Reading Issues
- The service will continue running with previous fan speeds if temperature cannot be read
- Check system logs for temperature sensor availability

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## License

This project follows the same license as the original uni-sync project (MIT).

## Acknowledgments

- [uni-sync](https://github.com/EightB1ts/uni-sync) - The original tool this project is built upon
- [EightB1ts](https://github.com/EightB1ts) - Creator of uni-sync