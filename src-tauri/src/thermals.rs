use std::{fs, path::Path};

use crate::models::TemperatureReading;

pub fn collect_temperatures(warnings: &mut Vec<String>) -> Vec<TemperatureReading> {
    let mut readings = Vec::new();
    let root = Path::new("/sys/class/thermal");

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(format!("temperature collector: {error}"));
            return readings;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        if !name.starts_with("thermal_zone") {
            continue;
        }

        let type_path = path.join("type");
        let temp_path = path.join("temp");

        let label = fs::read_to_string(&type_path)
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|_| name.to_string());

        let raw_temp = match fs::read_to_string(&temp_path) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let temp_millic = match raw_temp.trim().parse::<i64>() {
            Ok(value) => value,
            Err(_) => continue,
        };

        let celsius = temp_millic as f32 / 1000.0;
        if !(0.0..=150.0).contains(&celsius) {
            continue;
        }

        readings.push(TemperatureReading { label, celsius });
    }

    readings.sort_by(|left, right| left.label.cmp(&right.label));
    readings
}
