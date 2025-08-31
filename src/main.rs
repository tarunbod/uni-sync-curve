mod hardware;
mod config;
mod curve;

use anyhow::{bail, Result};
use clap::Parser;
use std::path::Path;
use std::time::Duration;
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let mut fan_controller = hardware::FanController::new()?;
    let available_devices = fan_controller.get_available_devices();
    if args.debug {
        println!("Available devices: {:?}", available_devices);
    }

    if available_devices.is_empty() {
        bail!("No Lian Li UNI devices found. Please ensure your devices are connected and you have the necessary permissions.");
    }

    let config_path = args
        .config_file
        .as_deref()
        .unwrap_or("/etc/uni-sync-curve.json");

    let config = config::load_config(Path::new(config_path), available_devices)?;

    println!("Using config file: {}", config_path);
    println!(
        "Loaded configuration with {} fan curves",
        config.fan_curves.len()
    );
    println!("Update interval: {} seconds", config.interval_seconds);

    let mut interval = time::interval(Duration::from_secs(config.interval_seconds));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("Exiting.");
                break;
            }
            _ = interval.tick() => {}
        }

        match hardware::get_max_cpu_temperature() {
            Some(cpu_temp) => {
                if args.debug {
                    println!("CPU temp: {:.1}°C", cpu_temp);
                }
                for fan_curve in &config.fan_curves {
                    let speed = curve::calculate_fan_speed(fan_curve, cpu_temp);
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
                    ).await {
                        eprintln!("Error applying fan speed: {}", e);
                    }
                }
            }
            _ => eprintln!("Could not read CPU temperature. Continuing with previous settings."),
        }
    }

    Ok(())
}
