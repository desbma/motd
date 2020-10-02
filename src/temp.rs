use std::cmp;
use std::collections::HashSet;
use std::error;
use std::fs::{self, File};
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use std::str::FromStr;

use ansi_term::Colour::*;
use glob::glob;
use simple_error::SimpleError;

/// Type of temperature sensor
enum SensorType {
    /// CPU sensor
    CPU,
    /// Hard drive or SSD/NVM sensor
    DRIVE,
    /// Other sensors (typically motherboard)
    OTHER,
}

/// Temperature data
pub struct SensorTemp {
    /// Name of sensor
    name: String,
    /// Type of sensor
    #[allow(dead_code)]
    sensor_type: SensorType,
    /// Temperature value in Celcius
    temp: u32,
    /// Temperature above which component is considered anormally hot
    temp_warning: u32,
    /// Temperature above which component is considered critically hot
    temp_critical: u32,
}

/// Deque of fetched temperature data
pub type TempDeque = Vec<SensorTemp>;

/// Read temperature from a given hwmon sysfs file
fn read_sysfs_temp_value(filepath: String) -> Result<Option<u32>, Box<dyn error::Error>> {
    let mut input_file = match File::open(filepath) {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };
    let mut temp_str = String::new();
    input_file.read_to_string(&mut temp_str)?;
    let temp_val = temp_str.trim_end().parse::<i32>().map(|v| v / 1000)?;

    if temp_val <= 0 {
        // Exclude negative values
        return Ok(None);
    }

    Ok(Some(temp_val as u32))
}

/// Probe temperatures from hwmon Linux sensors exposed in /sys/class/hwmon/
pub fn get_hwmon_temps() -> Result<TempDeque, Box<dyn error::Error>> {
    let mut temps = Vec::new();

    // Totally incomplete and arbitary list of sensor names to blacklist
    // = those that return invalid values on motherboards I own
    let mut label_blacklist: HashSet<String> = HashSet::new();
    label_blacklist.insert("SYSTIN".to_string());
    label_blacklist.insert("CPUTIN".to_string());

    for hwmon_entry in glob("/sys/class/hwmon/hwmon*")? {
        let hwmon_dir = hwmon_entry?
            .into_os_string()
            .into_string()
            .or_else(|_| Err(SimpleError::new("Failed to convert OS string")))?;
        let label_pattern = format!("{}/temp*_label", hwmon_dir);
        for label_entry in glob(&label_pattern).unwrap() {
            // Read sensor name
            let input_label_filepath = label_entry?
                .into_os_string()
                .into_string()
                .or_else(|_| Err(SimpleError::new("Failed to convert OS string")))?;
            let mut label = String::new();
            let mut input_label_file = File::open(&input_label_filepath)?;
            input_label_file.read_to_string(&mut label)?;
            label = label.trim_end().to_string();
            if label_blacklist.contains(&label) {
                // Label in blacklist, exclude
                continue;
            }

            // Deduce type from name
            let sensor_type = if label.starts_with("CPU ") || label.starts_with("Core ") {
                SensorType::CPU
            } else {
                SensorType::OTHER
            };

            // Read temp
            let input_temp_filepath = format!(
                "{}_input",
                input_label_filepath[..input_label_filepath.len() - 6].to_owned()
            );
            let temp_val = match read_sysfs_temp_value(input_temp_filepath)? {
                Some(v) => v,
                None => continue,
            };

            // Read warning temp
            let max_temp_filepath = format!(
                "{}_max",
                input_label_filepath[..input_label_filepath.len() - 6].to_owned()
            );
            let max_temp_val = read_sysfs_temp_value(max_temp_filepath)?;

            // Read critical temp
            let crit_temp_filepath = format!(
                "{}_crit",
                input_label_filepath[..input_label_filepath.len() - 6].to_owned()
            );
            let crit_temp_val = read_sysfs_temp_value(crit_temp_filepath)?;

            // Compute warning & critical temps
            let warning_temp;
            let crit_temp;
            if let (Some(max_temp_val), Some(crit_temp_val)) = (max_temp_val, crit_temp_val) {
                let (mut max_temp_val, crit_temp_val) = (
                    cmp::min(max_temp_val, crit_temp_val),
                    cmp::max(max_temp_val, crit_temp_val),
                );
                let abs_diff = crit_temp_val - max_temp_val;
                let delta = match sensor_type {
                    SensorType::CPU => abs_diff / 2,
                    SensorType::OTHER => 5,
                    _ => unreachable!(),
                };
                if let SensorType::OTHER = sensor_type {
                    if abs_diff > 20 {
                        max_temp_val = crit_temp_val - 20;
                    }
                }
                warning_temp = max_temp_val - delta;
                crit_temp = max_temp_val;
            } else if let Some(max_temp_val) = max_temp_val {
                let delta = match sensor_type {
                    SensorType::CPU => 10,
                    SensorType::OTHER => 5,
                    _ => unreachable!(),
                };
                warning_temp = max_temp_val - delta;
                crit_temp = max_temp_val;
            } else {
                warning_temp = match sensor_type {
                    // Fallback to default value
                    SensorType::CPU => 60,
                    SensorType::OTHER => 50,
                    _ => unreachable!(),
                };
                crit_temp = match sensor_type {
                    // Fallback to default value
                    SensorType::CPU => 75,
                    SensorType::OTHER => 60,
                    _ => unreachable!(),
                };
            }

            // Store temp
            let sensor_temp = SensorTemp {
                name: label,
                sensor_type,
                temp: temp_val,
                temp_warning: warning_temp,
                temp_critical: crit_temp,
            };
            temps.push(sensor_temp);
        }
    }

    Ok(temps)
}

/// Normalize a drive device path by making it absolute and following links
fn normalize_drive_path(path: &str) -> Result<String, Box<dyn error::Error>> {
    let mut path_string = path.to_string();
    let fs_path = Path::new(path);

    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        let mut real_path = fs::read_link(path)?;
        if !real_path.is_absolute() {
            let dirname = fs_path
                .parent()
                .ok_or_else(|| SimpleError::new("Unable to get drive parent directory"))?;
            real_path = dirname.join(real_path).canonicalize()?;
        }
        path_string = real_path
            .into_os_string()
            .into_string()
            .or_else(|_| Err(SimpleError::new("Failed to convert OS string")))?;
    }

    Ok(path_string)
}

/// Probe drive temperatures from hddtemp daemon
pub fn get_drive_temps() -> Result<TempDeque, Box<dyn error::Error>> {
    let mut temps = Vec::new();

    // Connect
    let mut stream = match TcpStream::connect("127.0.0.1:7634") {
        // TODO port const
        Ok(s) => s,
        Err(_) => return Ok(temps),
    };

    // Read
    let mut data = String::new();
    stream.read_to_string(&mut data)?;

    // Parse
    let drives_data: Vec<&str> = data.split('|').collect();
    for drive_data in drives_data.chunks_exact(5) {
        let drive_path = normalize_drive_path(drive_data[1])?;
        let pretty_name = drive_data[2];
        let temp = match u32::from_str(drive_data[3]) {
            Ok(t) => t,
            Err(_) => continue,
        };

        // Store temp
        let sensor_temp = SensorTemp {
            name: format!("{} ({})", drive_path, pretty_name),
            sensor_type: SensorType::DRIVE,
            temp,
            temp_warning: 45,
            temp_critical: 55,
        };
        temps.push(sensor_temp);
    }

    Ok(temps)
}

/// Colorize a string for terminal display according to temperature level
fn colorize_from_temp(string: String, temp: u32, temp_warning: u32, temp_critical: u32) -> String {
    if temp >= temp_critical {
        Red.paint(string).to_string()
    } else if temp >= temp_warning {
        Yellow.paint(string).to_string()
    } else {
        string
    }
}

/// Output all temperatures
pub fn output_temps(temps: TempDeque) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    let max_name_len = temps.iter().map(|x| x.name.len()).max().unwrap();
    for sensor_temp in temps {
        let pad = " ".repeat(max_name_len - sensor_temp.name.len());
        let line = format!("{}: {}{} °C", sensor_temp.name, pad, sensor_temp.temp);
        lines.push(colorize_from_temp(
            line,
            sensor_temp.temp,
            sensor_temp.temp_warning,
            sensor_temp.temp_critical,
        ));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_temps() {
        assert_eq!(
            output_temps(vec![
                SensorTemp {
                    name: "sensor1".to_string(),
                    sensor_type: SensorType::CPU,
                    temp: 95,
                    temp_warning: 70,
                    temp_critical: 80
                },
                SensorTemp {
                    name: "sensor222222222".to_string(),
                    sensor_type: SensorType::DRIVE,
                    temp: 40,
                    temp_warning: 70,
                    temp_critical: 80
                },
                SensorTemp {
                    name: "sensor333".to_string(),
                    sensor_type: SensorType::OTHER,
                    temp: 50,
                    temp_warning: 45,
                    temp_critical: 60
                }
            ]),
            [
                "\u{1b}[31msensor1:         95 °C\u{1b}[0m",
                "sensor222222222: 40 °C",
                "\u{1b}[33msensor333:       50 °C\u{1b}[0m"
            ]
        );
    }

    #[test]
    fn test_colorize_from_temp() {
        assert_eq!(colorize_from_temp("hey".to_string(), 59, 60, 75), "hey");
        assert_eq!(
            colorize_from_temp("hey".to_string(), 60, 60, 75),
            "\u{1b}[33mhey\u{1b}[0m"
        );
        assert_eq!(
            colorize_from_temp("hey".to_string(), 60, 60, 75),
            "\u{1b}[33mhey\u{1b}[0m"
        );
        assert_eq!(
            colorize_from_temp("hey".to_string(), 74, 60, 75),
            "\u{1b}[33mhey\u{1b}[0m"
        );
        assert_eq!(
            colorize_from_temp("hey".to_string(), 75, 60, 75),
            "\u{1b}[31mhey\u{1b}[0m"
        );
        assert_eq!(
            colorize_from_temp("hey".to_string(), 76, 60, 75),
            "\u{1b}[31mhey\u{1b}[0m"
        );
    }
}
