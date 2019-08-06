use std::cmp;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use std::str::FromStr;

use ansi_term::Colour::*;
use glob::glob;

/// Type of temperature sensor
#[derive(PartialEq)]
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
pub type TempDeque = VecDeque<SensorTemp>;

/// Read temperature from a given hwmon sysfs file
fn read_sysfs_temp_value(filepath: String) -> Option<u32> {
    let mut input_file = match File::open(filepath) {
        Ok(f) => f,
        Err(_e) => return None,
    };
    let mut temp_str = String::new();
    input_file.read_to_string(&mut temp_str).unwrap();
    let temp_val = match temp_str.trim_end().parse::<u32>() {
        Ok(v) => v / 1000,
        Err(_e) => return None,
    };
    if temp_val == 0 {
        // Exclude negative values
        return None;
    }

    Some(temp_val)
}

/// Probe temperatures from hwmon Linux sensors exposed in /sys/class/hwmon/
pub fn get_hwmon_temps(temps: &mut TempDeque) {
    // Totally incomplete and arbitary list of sensor names to blacklist
    // = those that return invalid values on motherboards I own
    let mut label_blacklist: HashSet<String> = HashSet::new();
    label_blacklist.insert("SYSTIN".to_string());
    label_blacklist.insert("CPUTIN".to_string());

    for hwmon_entry in glob("/sys/class/hwmon/hwmon*").unwrap() {
        let hwmon_dir = hwmon_entry.unwrap().into_os_string().into_string().unwrap();
        let label_pattern = format!("{}/temp*_label", hwmon_dir);
        for label_entry in glob(&label_pattern).unwrap() {
            // Read sensor name
            let input_label_filepath = label_entry.unwrap().into_os_string().into_string().unwrap();
            let mut label = String::new();
            let mut input_label_file = File::open(&input_label_filepath).unwrap();
            input_label_file.read_to_string(&mut label).unwrap();
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
            let temp_val = match read_sysfs_temp_value(input_temp_filepath) {
                Some(v) => v,
                None => continue,
            };

            // Read warning temp
            let max_temp_filepath = format!(
                "{}_max",
                input_label_filepath[..input_label_filepath.len() - 6].to_owned()
            );
            let max_temp_val = read_sysfs_temp_value(max_temp_filepath);

            // Read critical temp
            let crit_temp_filepath = format!(
                "{}_crit",
                input_label_filepath[..input_label_filepath.len() - 6].to_owned()
            );
            let crit_temp_val = read_sysfs_temp_value(crit_temp_filepath);

            // Compute warning & critical temps
            let warning_temp;
            let crit_temp;
            if max_temp_val.is_some() && crit_temp_val.is_some() {
                let (mut max_temp_val, crit_temp_val) = (
                    cmp::min(max_temp_val.unwrap(), crit_temp_val.unwrap()),
                    cmp::max(max_temp_val.unwrap(), crit_temp_val.unwrap()),
                );
                let abs_diff = crit_temp_val - max_temp_val;
                let delta = match sensor_type {
                    SensorType::CPU => abs_diff / 2,
                    SensorType::OTHER => 5,
                    _ => panic!(),
                };
                if (sensor_type == SensorType::OTHER) && (abs_diff > 20) {
                    max_temp_val = crit_temp_val - 20;
                }
                warning_temp = max_temp_val - delta;
                crit_temp = max_temp_val;
            } else if max_temp_val.is_some() {
                let max_temp_val = max_temp_val.unwrap();
                let delta = match sensor_type {
                    SensorType::CPU => 10,
                    SensorType::OTHER => 5,
                    _ => panic!(),
                };
                warning_temp = max_temp_val - delta;
                crit_temp = max_temp_val;
            } else {
                warning_temp = match sensor_type {
                    // Fallback to default value
                    SensorType::CPU => 60,
                    SensorType::OTHER => 50,
                    _ => panic!(),
                };
                crit_temp = match sensor_type {
                    // Fallback to default value
                    SensorType::CPU => 75,
                    SensorType::OTHER => 60,
                    _ => panic!(),
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
            temps.push_back(sensor_temp);
        }
    }
}

/// Normalize a drive device path by making it absolute and following links
fn normalize_drive_path(path: &str) -> String {
    let mut path_string = path.to_string();
    let fs_path = Path::new(path);

    if fs::symlink_metadata(path).unwrap().file_type().is_symlink() {
        let mut real_path = fs::read_link(path).unwrap();
        if !real_path.is_absolute() {
            let dirname = fs_path.parent().unwrap();
            real_path = dirname.join(real_path).canonicalize().unwrap();
        }
        path_string = real_path.into_os_string().into_string().unwrap();
    }

    path_string
}

/// Probe drive temperatures from hddtemp daemon
pub fn get_drive_temps(temps: &mut TempDeque) {
    // Connect
    let mut stream = match TcpStream::connect("127.0.0.1:7634") {
        // TODO port const
        Ok(s) => s,
        Err(_e) => return,
    };

    // Read
    let mut data = String::new();
    stream.read_to_string(&mut data).unwrap();

    // Parse
    let drives_data: Vec<&str> = data.split('|').collect();
    for drive_data in drives_data.chunks_exact(5) {
        let drive_path = normalize_drive_path(drive_data[1]);
        let pretty_name = drive_data[2];
        let temp = match u32::from_str(drive_data[3]) {
            Ok(t) => t,
            Err(_e) => continue,
        };

        // Store temp
        let sensor_temp = SensorTemp {
            name: format!("{} ({})", drive_path, pretty_name),
            sensor_type: SensorType::DRIVE,
            temp,
            temp_warning: 45,
            temp_critical: 55,
        };
        temps.push_back(sensor_temp);
    }
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
pub fn output_temps(temps: TempDeque) -> VecDeque<String> {
    let mut lines: VecDeque<String> = VecDeque::new();

    let max_name_len = temps
        .iter()
        .max_by_key(|x| x.name.len())
        .unwrap()
        .name
        .len();
    for sensor_temp in temps {
        let pad = " ".repeat(max_name_len - sensor_temp.name.len());
        let line = format!("{}: {}{} °C", sensor_temp.name, pad, sensor_temp.temp);
        lines.push_back(colorize_from_temp(
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
            output_temps(TempDeque::from(vec![
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
            ])),
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
