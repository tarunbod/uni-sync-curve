use crate::config::FanCurve;

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
    use crate::config::ChannelMode;
    use crate::config::CurvePoint;
    use crate::config::DeviceId;

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
