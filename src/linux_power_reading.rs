use std::{
    fs, io,
    path::{Path, PathBuf},
};

const HWMON_PATH: &str = "/sys/class/hwmon";
const BATTERY_CAPACITY_PATH: &str = "/sys/class/power_supply/macsmc-battery/capacity";
const AC_ONLINE_PATH: &str = "/sys/class/power_supply/macsmc-ac/online";

#[derive(Clone, Copy)]
pub struct Readings {
    pub voltage: f32,
    pub current: f32,
    pub system_total: f32,
    pub on_ac_power: bool,
    pub battery_percentage: Option<f32>,
}

pub struct PowerReader;

impl PowerReader {
    pub fn open() -> Result<Self, io::Error> {
        Ok(Self)
    }

    pub fn read_power(&self) -> Result<Readings, io::Error> {
        let voltage = read_labeled_sensor(
            |label| label.contains("dc in") || label.contains("input voltage"),
            "in",
            1_000.0,
        )?;
        let current = read_labeled_sensor(
            |label| label.contains("dc in") || label.contains("input current"),
            "curr",
            1_000.0,
        )?;
        let system_total = read_labeled_sensor(
            |label| label.contains("total system power"),
            "power",
            1_000_000.0,
        )?;
        let on_ac_power = read_integer(AC_ONLINE_PATH)? != 0;
        let battery_percentage = read_integer(BATTERY_CAPACITY_PATH)
            .ok()
            .map(|percentage| percentage.clamp(0, 100) as f32);

        Ok(Readings {
            voltage,
            current,
            system_total,
            on_ac_power,
            battery_percentage,
        })
    }
}

fn read_labeled_sensor(
    matches_label: impl Fn(&str) -> bool,
    sensor_prefix: &str,
    divisor: f32,
) -> Result<f32, io::Error> {
    let hwmon = fs::read_dir(HWMON_PATH).map_err(|error| {
        io::Error::other(format!("Unable to scan Linux hwmon sensors: {error}"))
    })?;

    for device in hwmon {
        let device = device?;
        let Ok(files) = fs::read_dir(device.path()) else {
            continue;
        };

        for file in files.flatten() {
            let label_path = file.path();
            let Some(name) = label_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(sensor_name) = name.strip_suffix("_label") else {
                continue;
            };
            if !sensor_name.starts_with(sensor_prefix) {
                continue;
            }

            let Ok(label) = fs::read_to_string(&label_path) else {
                continue;
            };
            if !matches_label(&label.to_ascii_lowercase()) {
                continue;
            }

            let input_path = input_path_for_label(&label_path, sensor_name);
            let raw_value = fs::read_to_string(&input_path).map_err(|error| {
                io::Error::other(format!(
                    "Unable to read Linux power sensor {}: {error}",
                    input_path.display()
                ))
            })?;
            let raw_value = raw_value.trim().parse::<f32>().map_err(|error| {
                io::Error::other(format!(
                    "Invalid numeric value in Linux power sensor {}: {error}",
                    input_path.display()
                ))
            })?;
            return Ok(raw_value / divisor);
        }
    }

    Err(io::Error::other(format!(
        "Unable to find matching Linux hwmon {sensor_prefix} sensor"
    )))
}

fn input_path_for_label(label_path: &Path, sensor_name: &str) -> PathBuf {
    label_path.with_file_name(format!("{sensor_name}_input"))
}

fn read_integer(path: &str) -> Result<i32, io::Error> {
    let value = fs::read_to_string(path)
        .map_err(|error| io::Error::other(format!("Unable to read {path}: {error}")))?;
    value
        .trim()
        .parse()
        .map_err(|error| io::Error::other(format!("Invalid numeric value in {path}: {error}")))
}
