use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Debug)]
// (vendor_id, product_id, serial_number)
pub struct DeviceId(pub u16, pub u16, pub String);

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


