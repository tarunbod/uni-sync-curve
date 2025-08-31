use anyhow::{anyhow, bail, Result};
use clap::Parser;
use hidapi::{self, HidDevice};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use sysinfo::Components;
use tokio::time;

#[derive(Parser, Debug)]
#[command(name = "uni-sync-curve")]
#[command(about = "A fan curve control daemon for Lian Li Uni fans")]
pub struct Args {
    #[arg(
        long = "config-file",
        help = "Path to configuration file (default: /etc/uni-sync-curve.json)"
    )]
    pub config_file: Option<String>,

    #[arg(long, help = "Enable debug logging")]
    pub debug: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Debug)]
pub struct DeviceId(u16, u16, String);

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:04x}, {:04x}, {})", self.0, self.1, self.2)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CurveConfig {
    pub interval_seconds: u64,
    pub fan_curves: Vec<FanCurve>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FanCurve {
    pub device_id: DeviceId,
    pub channel: usize,
    pub mode: ChannelMode,
    pub curve_points: Vec<CurvePoint>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ChannelMode {
    Manual,
    PWM,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CurvePoint {
    pub temperature_celsius: f64,
    pub fan_speed_percent: u8,
}

fn get_default_config(device_ids: Vec<DeviceId>) -> CurveConfig {
    CurveConfig {
        interval_seconds: 10,
        fan_curves: device_ids
            .into_iter()
            .map(|device_id| FanCurve {
                device_id,
                channel: 0,
                mode: ChannelMode::Manual,
                curve_points: vec![
                    CurvePoint {
                        temperature_celsius: 30.0,
                        fan_speed_percent: 25,
                    },
                    CurvePoint {
                        temperature_celsius: 50.0,
                        fan_speed_percent: 50,
                    },
                    CurvePoint {
                        temperature_celsius: 65.0,
                        fan_speed_percent: 75,
                    },
                    CurvePoint {
                        temperature_celsius: 80.0,
                        fan_speed_percent: 100,
                    },
                ],
            })
            .collect(),
    }
}

pub fn load_config(config_path: &Path, available_devices: Vec<DeviceId>) -> Result<CurveConfig> {
    if !config_path.exists() {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let default_config = get_default_config(available_devices);
        let config_json = serde_json::to_string_pretty(&default_config)?;
        std::fs::write(config_path, config_json)?;
        println!("Created default configuration at: {:?}", config_path);
        return Ok(default_config);
    }

    let config_content = std::fs::read_to_string(config_path)?;
    let config: CurveConfig = serde_json::from_str(&config_content)?;
    Ok(config)
}

const CPU_KEYWORDS: [&str; 4] = ["cpu", "core", "processor", "tctl"];

fn get_max_cpu_temperature() -> Option<f64> {
    let components = Components::new_with_refreshed_list();

    let mut max_temp = None;

    for component in &components {
        let name = component.label().to_lowercase();
        if CPU_KEYWORDS.iter().any(|&kw| name.contains(kw)) {
            let temp = component.temperature() as f64;
            match max_temp {
                None => max_temp = Some(temp),
                Some(current_max) => {
                    if temp > current_max {
                        max_temp = Some(temp);
                    }
                }
            }
        }
    }

    max_temp
}

pub fn calculate_fan_speed(curve: &FanCurve, temperature: f64) -> u8 {
    let points = &curve.curve_points;

    if points.is_empty() {
        return 50;
    }

    if points.len() == 1 {
        return points[0].fan_speed_percent;
    }

    let mut sorted_points = points.clone();
    sorted_points.sort_by(|a, b| {
        a.temperature_celsius
            .partial_cmp(&b.temperature_celsius)
            .unwrap()
    });

    if temperature <= sorted_points[0].temperature_celsius {
        return sorted_points[0].fan_speed_percent;
    }

    if temperature >= sorted_points.last().unwrap().temperature_celsius {
        return sorted_points.last().unwrap().fan_speed_percent;
    }

    for i in 0..sorted_points.len() - 1 {
        let point1 = &sorted_points[i];
        let point2 = &sorted_points[i + 1];

        if temperature >= point1.temperature_celsius && temperature <= point2.temperature_celsius {
            return interpolate(
                point1.temperature_celsius,
                point1.fan_speed_percent,
                point2.temperature_celsius,
                point2.fan_speed_percent,
                temperature,
            );
        }
    }

    50
}

fn interpolate(temp1: f64, speed1: u8, temp2: f64, speed2: u8, current_temp: f64) -> u8 {
    let temp_range = temp2 - temp1;
    let speed_range = speed2 as f64 - speed1 as f64;
    let temp_offset = current_temp - temp1;

    let interpolated_speed = speed1 as f64 + (temp_offset / temp_range) * speed_range;

    interpolated_speed.round().clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fan_curve_calculation() {
        let curve = FanCurve {
            device_id: DeviceId(0x0cf2, 0x7750, "TEST123".to_string()),
            channel: 0,
            mode: ChannelMode::Manual,
            curve_points: vec![
                CurvePoint {
                    temperature_celsius: 30.0,
                    fan_speed_percent: 20,
                },
                CurvePoint {
                    temperature_celsius: 50.0,
                    fan_speed_percent: 40,
                },
                CurvePoint {
                    temperature_celsius: 70.0,
                    fan_speed_percent: 70,
                },
                CurvePoint {
                    temperature_celsius: 85.0,
                    fan_speed_percent: 100,
                },
            ],
        };

        assert_eq!(calculate_fan_speed(&curve, 25.0), 20);
        assert_eq!(calculate_fan_speed(&curve, 30.0), 20);
        assert_eq!(calculate_fan_speed(&curve, 40.0), 30);
        assert_eq!(calculate_fan_speed(&curve, 50.0), 40);
        assert_eq!(calculate_fan_speed(&curve, 60.0), 55);
        assert_eq!(calculate_fan_speed(&curve, 70.0), 70);
        assert_eq!(calculate_fan_speed(&curve, 90.0), 100);
    }
}

// Lian Li Uni-Sync Fans - Vendor ID and Product IDs
const VENDOR_IDS: [u16; 1] = [0x0cf2];
const PRODUCT_IDS: [u16; 7] = [0x7750, 0xa100, 0xa101, 0xa102, 0xa103, 0xa104, 0xa105];

pub struct FanController {
    hidapi: hidapi::HidApi,
    device_configs: HashMap<DeviceId, hidapi::DeviceInfo>,
}

impl FanController {
    pub fn new() -> Result<Self> {
        let hidapi = hidapi::HidApi::new()?;
        let device_configs = hidapi
            .device_list()
            .filter_map(|d| {
                if VENDOR_IDS.contains(&d.vendor_id()) && PRODUCT_IDS.contains(&d.product_id()) {
                    Some((
                        DeviceId(
                            d.vendor_id(),
                            d.product_id(),
                            d.serial_number()?.to_string(),
                        ),
                        d.clone(),
                    ))
                } else {
                    None
                }
            })
            .collect();

        Ok(Self {
            hidapi,
            device_configs,
        })
    }

    pub fn set_fan_speed(
        &mut self,
        device_id: &DeviceId,
        channel: usize,
        mode: &ChannelMode,
        speed_percent: u8,
    ) -> Result<()> {
        let hiddevice = self
            .device_configs
            .get(device_id)
            .ok_or_else(|| anyhow!("Device with given device id {} not available", device_id))?;

        let hid: HidDevice = match self.hidapi.open_path(hiddevice.path()) {
            Ok(hid) => hid,
            Err(_) => {
                eprintln!("Please run uni-sync with elevated permissions.");
                std::process::exit(0);
            }
        };

        let sync_rgb: bool = false;

        // Send Command to Sync to RGB Header
        let sync_byte: u8 = if sync_rgb { 1 } else { 0 };
        let _ = match &hiddevice.product_id() {
            0xa100 | 0x7750 => hid.write(&[224, 16, 48, sync_byte, 0, 0, 0]), // SL
            0xa101 => hid.write(&[224, 16, 65, sync_byte, 0, 0, 0]),          // AL
            0xa102 => hid.write(&[224, 16, 97, sync_byte, 0, 0, 0]),          // SLI
            0xa103 | 0xa105 => hid.write(&[224, 16, 97, sync_byte, 0, 0, 0]), // SLv2
            0xa104 => hid.write(&[224, 16, 97, sync_byte, 0, 0, 0]),          // ALv2
            _ => hid.write(&[224, 16, 48, sync_byte, 0, 0, 0]),               // SL
        };

        // Avoid Race Condition
        std::thread::sleep(time::Duration::from_millis(200));

        // Disable Sync to fan header
        let mut channel_byte = 0x10 << channel;
        if matches!(mode, ChannelMode::PWM) {
            channel_byte |= 0x1 << channel;
        }

        let _ = match &hiddevice.product_id() {
            0xa100 | 0x7750 => hid.write(&[224, 16, 49, channel_byte]), // SL
            0xa101 => hid.write(&[224, 16, 66, channel_byte]),          // AL
            0xa102 => hid.write(&[224, 16, 98, channel_byte]),          // SLI
            0xa103 | 0xa105 => hid.write(&[224, 16, 98, channel_byte]), // SLv2
            0xa104 => hid.write(&[224, 16, 98, channel_byte]),          // ALv2
            _ => hid.write(&[224, 16, 49, channel_byte]),               // SL
        };

        // Avoid Race Condition
        std::thread::sleep(time::Duration::from_millis(200));

        // Set Channel Speed
        if matches!(mode, ChannelMode::Manual) {
            let speed = (speed_percent as f64).clamp(0.0, 100.0);

            let speed_800_1900: u8 = ((800.0 + (11.0 * speed)) as usize / 19).try_into().unwrap();
            let speed_250_2000: u8 = ((250.0 + (17.5 * speed)) as usize / 20).try_into().unwrap();
            let speed_200_2100: u8 = ((200.0 + (19.0 * speed)) as usize / 21).try_into().unwrap();

            let _ = match &hiddevice.product_id() {
                0xa100 | 0x7750 => {
                    hid.write(&[224, (channel + 32).try_into().unwrap(), 0, speed_800_1900])
                } // SL
                0xa101 => hid.write(&[224, (channel + 32).try_into().unwrap(), 0, speed_800_1900]), // AL
                0xa102 => hid.write(&[224, (channel + 32).try_into().unwrap(), 0, speed_200_2100]), // SLI
                0xa103 | 0xa105 => {
                    hid.write(&[224, (channel + 32).try_into().unwrap(), 0, speed_250_2000])
                } // SLv2
                0xa104 => hid.write(&[224, (channel + 32).try_into().unwrap(), 0, speed_250_2000]), // ALv2
                _ => hid.write(&[224, (channel + 32).try_into().unwrap(), 0, speed_800_1900]), // SL
            };

            // Avoid Race Condition
            std::thread::sleep(time::Duration::from_millis(100));
        }

        Ok(())
    }

    pub fn get_available_devices(&self) -> Vec<DeviceId> {
        self.device_configs.keys().cloned().collect()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let mut fan_controller = FanController::new()?;
    let available_devices = fan_controller.get_available_devices();
    if available_devices.is_empty() {
        bail!("No Lian Li UNI devices found. Please ensure your devices are connected and you have the necessary permissions.");
    }

    if args.debug {
        println!("Available devices: {:?}", available_devices);
    }

    let config_path = args
        .config_file
        .as_deref()
        .unwrap_or("/etc/uni-sync-curve.json");

    let config = load_config(Path::new(config_path), available_devices)?;

    println!("Using config file: {}", config_path);
    println!(
        "Loaded configuration with {} fan curves",
        config.fan_curves.len()
    );
    println!("Update interval: {} seconds", config.interval_seconds);

    let mut interval = time::interval(Duration::from_secs(config.interval_seconds));
    loop {
        interval.tick().await;

        match get_max_cpu_temperature() {
            Some(cpu_temp) => {
                if args.debug {
                    println!("CPU temp: {:.1}°C", cpu_temp);
                }
                for fan_curve in &config.fan_curves {
                    let speed = calculate_fan_speed(fan_curve, cpu_temp);
                    if args.debug {
                        println!(
                            "Setting device {} channel {} to {}%",
                            fan_curve.device_id, fan_curve.channel, speed
                        );
                    }

                    if let Err(e) = fan_controller.set_fan_speed(
                        &fan_curve.device_id,
                        fan_curve.channel,
                        &fan_curve.mode,
                        speed,
                    ) {
                        eprintln!("Error applying fan speed: {}", e);
                    }
                }
            }
            _ => eprintln!("Could not read CPU temperature. Continuing with previous settings."),
        }
    }
}
