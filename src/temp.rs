use std::{
    cmp, fmt, fs,
    io::prelude::*,
    net::TcpStream,
    path::{Path, PathBuf},
    str::FromStr,
};

use ansi_term::Colour::{Red, Yellow};
use anyhow::Context;

use crate::{config, ModuleData};

/// Type of temperature sensor
#[derive(Debug, PartialEq, Eq)]
enum SensorType {
    /// CPU sensor
    Cpu,
    /// Hard drive or SSD/NVM sensor
    Drive,
    /// Other sensors (typically motherboard), or we just have no clue
    OtherOrUnknown,
}

/// Temperature data
pub(crate) struct SensorTemp {
    /// Name of sensor
    name: String,
    /// Type of sensor
    #[expect(dead_code)]
    sensor_type: SensorType,
    /// Temperature value in Celcius
    temp: u32,
    /// Temperature above which component is considered anormally hot
    temp_warning: u32,
    /// Temperature above which component is considered critically hot
    temp_critical: u32,
}

/// Deque of fetched temperature data
pub(crate) struct HardwareTemps {
    temps: Vec<SensorTemp>,
}

/// Read temperature from a given hwmon sysfs file
fn read_sysfs_temp_value(filepath: &Path) -> anyhow::Result<u32> {
    let temp_str = read_sysfs_string_value(filepath)?;
    let temp_val = temp_str.trim_end().parse::<u32>().map(|v| v / 1000)?;

    anyhow::ensure!(temp_val > 0);

    Ok(temp_val)
}

/// Read string from a given sysfs file
fn read_sysfs_string_value(filepath: &Path) -> anyhow::Result<String> {
    Ok(fs::read_to_string(filepath)
        .with_context(|| format!("Failed to read {filepath:?}"))?
        .trim_end()
        .to_owned())
}

/// Probe temperatures from hwmon Linux sensors
#[expect(clippy::string_slice, clippy::too_many_lines)]
pub(crate) fn fetch(cfg: &config::TempConfig) -> anyhow::Result<ModuleData> {
    let mut temps = Vec::new();

    //
    // Hwmon sensors
    //

    let re = regex::Regex::new("temp[0-9]+_input").unwrap();

    for input_temp_filepath in walkdir::WalkDir::new("/sys/class/hwmon")
        .follow_links(true)
        .min_depth(2)
        .max_depth(2)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| !e.path_is_symlink() && e.file_type().is_file())
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|p| re.is_match(p.file_name().unwrap().to_str().unwrap()))
    {
        let input_temp_filepath_str = input_temp_filepath.to_str().unwrap();
        let filepath_prefix =
            input_temp_filepath_str[..input_temp_filepath_str.len() - 6].to_owned();

        // Read sensor label
        let label_filepath = PathBuf::from(&format!("{filepath_prefix}_label"));
        let label = if label_filepath.is_file() {
            let label = read_sysfs_string_value(&label_filepath)?;
            // Exclude from label blacklist
            if cfg.hwmon_label_blacklist.iter().any(|r| r.is_match(&label)) {
                continue;
            }
            Some(label)
        } else {
            None
        };

        // Get sensor driver name
        let name_filepath = input_temp_filepath.with_file_name("name");
        let name = read_sysfs_string_value(&name_filepath)?;

        // Deduce type from name
        let sensor_type = if let Some(label) = label.as_ref() {
            if label.starts_with("CPU ") || label.starts_with("Core ") {
                SensorType::Cpu
            } else {
                SensorType::OtherOrUnknown
            }
        } else if name == "drivetemp" {
            SensorType::Drive
        } else {
            SensorType::OtherOrUnknown
        };

        // Set drivetemp label
        let sensor_name = if let Some(label) = label {
            label
        } else if sensor_type == SensorType::Drive {
            let model_filepath = input_temp_filepath.with_file_name("device/model");
            let model = read_sysfs_string_value(&model_filepath)?;
            let block_dirpath = input_temp_filepath.with_file_name("device/block");
            let block_device_name = fs::read_dir(&block_dirpath)?
                .next()
                .ok_or_else(|| {
                    anyhow::anyhow!("Unable to get block device from {:?}", block_dirpath)
                })??
                .file_name()
                .into_string()
                .map_err(|e| anyhow::anyhow!("Unable to decode {:?}", e))?;
            format!("{block_device_name} ({model})")
        } else {
            name
        };

        // Read temp
        #[expect(clippy::shadow_unrelated)]
        let input_temp_filepath = PathBuf::from(&format!("{filepath_prefix}_input"));
        let Ok(temp_val) = read_sysfs_temp_value(&input_temp_filepath) else {
            continue;
        };

        // Read warning temp
        let max_temp_filepath = PathBuf::from(&format!("{filepath_prefix}_max"));
        let max_temp_val = read_sysfs_temp_value(&max_temp_filepath).ok();

        // Read critical temp
        let crit_temp_filepath = PathBuf::from(format!("{filepath_prefix}_crit"));
        let crit_temp_val = read_sysfs_temp_value(&crit_temp_filepath).ok();

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
                SensorType::Cpu => abs_diff / 2,
                SensorType::Drive | SensorType::OtherOrUnknown => 5,
            };
            if let SensorType::OtherOrUnknown = sensor_type {
                if abs_diff > 20 {
                    max_temp_val = crit_temp_val - 20;
                }
            }
            warning_temp = max_temp_val - delta;
            crit_temp = max_temp_val;
        } else if let Some(max_temp_val) = max_temp_val {
            let delta = match sensor_type {
                SensorType::Cpu => 10,
                SensorType::Drive | SensorType::OtherOrUnknown => 5,
            };
            warning_temp = max_temp_val - delta;
            crit_temp = max_temp_val;
        } else {
            warning_temp = match sensor_type {
                // Fallback to default value
                SensorType::Cpu => 60,
                SensorType::Drive | SensorType::OtherOrUnknown => 50,
            };
            crit_temp = match sensor_type {
                // Fallback to default value
                SensorType::Cpu => 75,
                SensorType::Drive | SensorType::OtherOrUnknown => 60,
            };
        }

        // Store temp
        let sensor_temp = SensorTemp {
            name: sensor_name,
            sensor_type,
            temp: temp_val,
            temp_warning: warning_temp,
            temp_critical: crit_temp,
        };
        temps.push(sensor_temp);
    }

    //
    // HDD temps
    //

    // Connect
    if let Ok(mut stream) = TcpStream::connect("127.0.0.1:7634") {
        // TODO port const
        // Read
        let mut data = String::new();
        stream.read_to_string(&mut data)?;

        // Parse
        let drives_data: Vec<&str> = data.split('|').collect();
        for drive_data in drives_data.chunks_exact(5) {
            let drive_path = normalize_drive_path(&PathBuf::from(drive_data[1]))?;
            let pretty_name = drive_data[2];
            let Ok(temp) = u32::from_str(drive_data[3]) else {
                continue;
            };

            // Store temp
            let sensor_temp = SensorTemp {
                name: format!("{} ({})", drive_path.to_str().unwrap(), pretty_name),
                sensor_type: SensorType::Drive,
                temp,
                temp_warning: 45,
                temp_critical: 55,
            };
            temps.push(sensor_temp);
        }
    }

    Ok(ModuleData::HardwareTemps(HardwareTemps { temps }))
}

/// Normalize a drive device path by making it absolute and following links
fn normalize_drive_path(path: &Path) -> anyhow::Result<PathBuf> {
    let mut path_string = path.to_path_buf();
    let fs_path = Path::new(path);

    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        let mut real_path = fs::read_link(path)?;
        if !real_path.is_absolute() {
            let dirname = fs_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Unable to get drive parent directory"))?;
            real_path = dirname.join(real_path).canonicalize()?;
        }
        path_string = real_path;
    }

    Ok(path_string)
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

impl fmt::Display for HardwareTemps {
    /// Output all temperatures
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let max_name_len = self.temps.iter().map(|x| x.name.len()).max();
        for sensor_temp in &self.temps {
            let pad = " ".repeat(max_name_len.unwrap() - sensor_temp.name.len());
            let line = format!("{}: {}{} °C", sensor_temp.name, pad, sensor_temp.temp);
            writeln!(
                f,
                "{}",
                colorize_from_temp(
                    line,
                    sensor_temp.temp,
                    sensor_temp.temp_warning,
                    sensor_temp.temp_critical,
                )
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_temps() {
        assert_eq!(
            format!(
                "{}",
                HardwareTemps {
                    temps: vec![
                        SensorTemp {
                            name: "sensor1".to_owned(),
                            sensor_type: SensorType::Cpu,
                            temp: 95,
                            temp_warning: 70,
                            temp_critical: 80
                        },
                        SensorTemp {
                            name: "sensor222222222".to_owned(),
                            sensor_type: SensorType::Drive,
                            temp: 40,
                            temp_warning: 70,
                            temp_critical: 80
                        },
                        SensorTemp {
                            name: "sensor333".to_owned(),
                            sensor_type: SensorType::OtherOrUnknown,
                            temp: 50,
                            temp_warning: 45,
                            temp_critical: 60
                        }
                    ]
                }
            ),
            "\u{1b}[31msensor1:         95 °C\u{1b}[0m\nsensor222222222: 40 °C\n\u{1b}[33msensor333:       50 °C\u{1b}[0m\n"
        );
    }

    #[test]
    fn test_colorize_from_temp() {
        assert_eq!(colorize_from_temp("hey".to_owned(), 59, 60, 75), "hey");
        assert_eq!(
            colorize_from_temp("hey".to_owned(), 60, 60, 75),
            "\u{1b}[33mhey\u{1b}[0m"
        );
        assert_eq!(
            colorize_from_temp("hey".to_owned(), 60, 60, 75),
            "\u{1b}[33mhey\u{1b}[0m"
        );
        assert_eq!(
            colorize_from_temp("hey".to_owned(), 74, 60, 75),
            "\u{1b}[33mhey\u{1b}[0m"
        );
        assert_eq!(
            colorize_from_temp("hey".to_owned(), 75, 60, 75),
            "\u{1b}[31mhey\u{1b}[0m"
        );
        assert_eq!(
            colorize_from_temp("hey".to_owned(), 76, 60, 75),
            "\u{1b}[31mhey\u{1b}[0m"
        );
    }
}
