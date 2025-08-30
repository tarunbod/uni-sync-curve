use hidapi::{self, HidDevice};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use sysinfo::Components;
use tokio::time;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CurveConfig {
    pub interval_seconds: u64,
    pub fan_curves: Vec<FanCurve>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FanCurve {
    pub device_id: String,
    pub channel: usize,
    pub curve_points: Vec<CurvePoint>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CurvePoint {
    pub temperature_celsius: f64,
    pub fan_speed_percent: u8,
}

impl Default for CurveConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 5,
            fan_curves: vec![FanCurve {
                device_id: "example".to_string(),
                channel: 0,
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
            }],
        }
    }
}

const CONFIG_PATH: &str = "/etc/uni-sync-curve/uni-sync-curve.json";

pub fn load_config() -> Result<CurveConfig, Box<dyn std::error::Error>> {
    let config_path = Path::new(CONFIG_PATH);
    if !config_path.exists() {
        let default_config = CurveConfig::default();

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let config_json = serde_json::to_string_pretty(&default_config)?;
        std::fs::write(&config_path, config_json)?;

        println!("Created default configuration at: {:?}", config_path);
        return Ok(default_config);
    }

    let config_content = std::fs::read_to_string(&config_path)?;
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

    interpolated_speed.round().max(0.0).min(100.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fan_curve_calculation() {
        let curve = FanCurve {
            device_id: "test".to_string(),
            channel: 0,
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

#[derive(Serialize, Deserialize, Clone)]
pub struct Configs {
    pub configs: Vec<Config>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub device_id: String,
    pub sync_rgb: bool,
    pub channels: Vec<Channel>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Channel {
    pub mode: String,
    pub speed: usize,
}

// Lian Li Uni-Sync Fans - Vendor ID and Product IDs
const VENDOR_IDS: [u16; 1] = [0x0cf2];
const PRODUCT_IDS: [u16; 7] = [0x7750, 0xa100, 0xa101, 0xa102, 0xa103, 0xa104, 0xa105];

pub fn run(mut existing_configs: Configs) -> Configs {
    let mut default_channels: Vec<Channel> = Vec::new();
    for _x in 0..4 {
        default_channels.push(Channel {
            mode: "Manual".to_string(),
            speed: 50,
        });
    }

    // Get All Devices
    let api = match hidapi::HidApi::new() {
        Ok(api) => api,
        Err(_) => panic!("Could not find any controllers"),
    };

    for hiddevice in api.device_list() {
        if VENDOR_IDS.contains(&hiddevice.vendor_id())
            && PRODUCT_IDS.contains(&hiddevice.product_id())
        {
            let serial_number: &str = match hiddevice.serial_number() {
                Some(sn) => sn,
                None => {
                    println!("Serial number not available for device {:?}", hiddevice);
                    continue;
                }
            };

            let path: &str = match hiddevice.path() {
                p => p.to_str().unwrap_or("unknown"),
            };

            let device_id: String = format!(
                "VID:{}/PID:{}/SN:{}/PATH:{}",
                hiddevice.vendor_id().to_string(),
                hiddevice.product_id().to_string(),
                serial_number.to_string(),
                path.to_string()
            );
            let hid: HidDevice = match api.open_path(hiddevice.path()) {
                Ok(hid) => hid,
                Err(_) => {
                    println!("Please run uni-sync with elevated permissions.");
                    std::process::exit(0);
                }
            };
            let mut channels: Vec<Channel> = default_channels.clone();
            let mut sync_rgb: bool = false;

            println!("Found: {:?}", device_id);

            if let Some(config) = existing_configs
                .configs
                .iter()
                .find(|config| config.device_id == device_id)
            {
                channels = config.channels.clone();
                sync_rgb = config.sync_rgb;
            } else {
                existing_configs.configs.push(Config {
                    device_id,
                    sync_rgb: false,
                    channels: channels.clone(),
                });
            }

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

            for x in 0..channels.len() {
                // Disable Sync to fan header
                let mut channel_byte = 0x10 << x;

                if channels[x].mode == "PWM" {
                    channel_byte = channel_byte | 0x1 << x;
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
                if channels[x].mode == "Manual" {
                    let mut speed = channels[x].speed as f64;
                    if speed > 100.0 {
                        speed = 100.0
                    }

                    let speed_800_1900: u8 =
                        ((800.0 + (11.0 * speed)) as usize / 19).try_into().unwrap();
                    let speed_250_2000: u8 =
                        ((250.0 + (17.5 * speed)) as usize / 20).try_into().unwrap();
                    let speed_200_2100: u8 =
                        ((200.0 + (19.0 * speed)) as usize / 21).try_into().unwrap();

                    let _ = match &hiddevice.product_id() {
                        0xa100 | 0x7750 => {
                            hid.write(&[224, (x + 32).try_into().unwrap(), 0, speed_800_1900])
                        } // SL
                        0xa101 => {
                            hid.write(&[224, (x + 32).try_into().unwrap(), 0, speed_800_1900])
                        } // AL
                        0xa102 => {
                            hid.write(&[224, (x + 32).try_into().unwrap(), 0, speed_200_2100])
                        } // SLI
                        0xa103 | 0xa105 => {
                            hid.write(&[224, (x + 32).try_into().unwrap(), 0, speed_250_2000])
                        } // SLv2
                        0xa104 => {
                            hid.write(&[224, (x + 32).try_into().unwrap(), 0, speed_250_2000])
                        } // ALv2
                        _ => hid.write(&[224, (x + 32).try_into().unwrap(), 0, speed_800_1900]), // SL
                    };

                    // Avoid Race Condition
                    std::thread::sleep(time::Duration::from_millis(100));
                }
            }
        }
    }

    return existing_configs;
}

pub struct FanController {
    device_configs: HashMap<String, Config>,
}

impl FanController {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut controller = Self {
            device_configs: HashMap::new(),
        };

        controller.discover_devices()?;
        Ok(controller)
    }

    fn discover_devices(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let empty_configs = Configs { configs: vec![] };
        let discovered_configs = run(empty_configs);

        for config in discovered_configs.configs {
            self.device_configs.insert(config.device_id.clone(), config);
        }

        println!("Discovered {} Lian Li devices", self.device_configs.len());
        Ok(())
    }

    pub fn set_fan_speed(
        &mut self,
        device_id: &str,
        channel: usize,
        speed_percent: u8,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(config) = self.device_configs.get_mut(device_id) {
            if channel < config.channels.len() {
                config.channels[channel] = Channel {
                    mode: "Manual".to_string(),
                    speed: speed_percent as usize,
                };

                let single_config = Configs {
                    configs: vec![config.clone()],
                };

                run(single_config);
                println!(
                    "Set device {} channel {} to {}%",
                    device_id, channel, speed_percent
                );
            } else {
                return Err(
                    format!("Channel {} not found for device {}", channel, device_id).into(),
                );
            }
        } else {
            return Err(format!("Device {} not found", device_id).into());
        }

        Ok(())
    }

    pub fn apply_fan_curves(
        &mut self,
        fan_curves: &[FanCurve],
        fan_speeds: &HashMap<String, u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for curve in fan_curves {
            if let Some(&speed) = fan_speeds.get(&curve.device_id) {
                self.set_fan_speed(&curve.device_id, curve.channel, speed)?;
            }
        }
        Ok(())
    }

    pub fn get_available_devices(&self) -> Vec<String> {
        self.device_configs.keys().cloned().collect()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    println!(
        "Loaded configuration with {} fan curves",
        config.fan_curves.len()
    );
    println!("Update interval: {} seconds", config.interval_seconds);

    let mut fan_controller = FanController::new()?;

    let available_devices = fan_controller.get_available_devices();
    println!("Available devices: {:?}", available_devices);

    if available_devices.is_empty() {
        println!("No Lian Li devices found. Please ensure your devices are connected and you have the necessary permissions.");
        return Ok(());
    }

    let mut interval = time::interval(Duration::from_secs(config.interval_seconds));

    loop {
        interval.tick().await;

        match get_max_cpu_temperature() {
            Some(cpu_temp) => {
                println!("Current CPU temperature: {:.1}°C", cpu_temp);

                for fan_curve in &config.fan_curves {
                    let speed = calculate_fan_speed(fan_curve, cpu_temp);
                    println!(
                        "Device {} Channel {}: {:.1}°C -> {}%",
                        fan_curve.device_id, fan_curve.channel, cpu_temp, speed
                    );

                    if let Err(e) =
                        fan_controller.set_fan_speed(&fan_curve.device_id, fan_curve.channel, speed)
                    {
                        eprintln!("Error applying fan speed: {}", e);
                    }
                }
            }
            _ => eprintln!("Could not read CPU temperature. Continuing with previous settings."),
        }
    }
}
