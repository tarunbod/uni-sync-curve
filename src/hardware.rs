use anyhow::{anyhow, Result};
use hidapi::{self, HidDevice};
use std::collections::HashMap;
use sysinfo::Components;
use tokio::time;

use crate::config::{ChannelMode, DeviceId};

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

    pub async fn set_fan_speed(
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
        time::sleep(time::Duration::from_millis(200)).await;

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
        time::sleep(time::Duration::from_millis(200)).await;

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
            time::sleep(time::Duration::from_millis(100)).await;
        }

        Ok(())
    }

    pub fn get_available_devices(&self) -> Vec<DeviceId> {
        self.device_configs.keys().cloned().collect()
    }
}

const CPU_KEYWORDS: [&str; 4] = ["cpu", "core", "processor", "tctl"];

pub fn get_max_cpu_temperature() -> Option<f64> {
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
