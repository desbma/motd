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
    sensor_type: SensorType,
    /// Temperature value in Celcius
    temp: u32,
}


/// Deque of fetched temperature data
pub type TempDeque = VecDeque<SensorTemp>;


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
            let sensor_type;
            if label.starts_with("CPU ") || label.starts_with("Core ") {
                sensor_type = SensorType::CPU;
            }
            else {
                sensor_type = SensorType::OTHER;
            }

            // Read temp
            let input_temp_filepath = format!("{}_input", input_label_filepath[..input_label_filepath.len() - 6].to_owned());  // TODO optimize this?
            let mut input_temp_file = File::open(input_temp_filepath).unwrap();
            let mut temp_str = String::new();
            input_temp_file.read_to_string(&mut temp_str).unwrap();
            let temp_val = match temp_str.trim_end().parse::<u32>() {
                Ok(v) => v / 1000,
                Err(_e) => continue,  // Some sensors return negative values
            };
            if temp_val == 0 {
                // Exclude negative values
                continue;
            }

            // Store temp
            let sensor_temp = SensorTemp {name: label,
                                          sensor_type: sensor_type,
                                          temp: temp_val};
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
    let mut stream = match TcpStream::connect("127.0.0.1:7634") {  // TODO port const
        Ok(s) => s,
        Err(_e) => return,
    };

    // Read
    let mut data = String::new();
    stream.read_to_string(&mut data).unwrap();

    // Parse
    let drives_data: Vec<&str> = data.split("|").collect();
    for drive_data in drives_data.chunks_exact(5) {
        let drive_path = normalize_drive_path(drive_data[1]);
        let pretty_name = drive_data[2];
        let temp = u32::from_str(drive_data[3]).unwrap();

        // Store temp
        let sensor_temp = SensorTemp {name: format!("{} ({})", drive_path, pretty_name),
                                      sensor_type: SensorType::DRIVE,
                                      temp: temp};
        temps.push_back(sensor_temp);
    }
}


/// Colorize a string for terminal display according to temperature level
fn colorize_from_temp(string: String, temp: u32, sensor_type: SensorType) -> String {
    let warning_temp = match sensor_type {
        SensorType::CPU => 60,
        SensorType::DRIVE => 45,
        SensorType::OTHER => 50,
    };
    let critical_temp = match sensor_type {
        SensorType::CPU => 75,
        SensorType::DRIVE => 55,
        SensorType::OTHER => 60,
    };
    if temp >= critical_temp {
        Red.paint(string).to_string()
    }
    else if temp >= warning_temp {
        Yellow.paint(string).to_string()
    }
    else {
        string
    }
}


/// Output all temperatures
pub fn output_temps(temps: TempDeque) -> VecDeque<String> {
    let mut lines: VecDeque<String> = VecDeque::new();

    let max_name_len = temps.iter().max_by_key(|x| x.name.len()).unwrap().name.len();
    for sensor_temp in temps {
        let pad = " ".repeat(max_name_len - sensor_temp.name.len());
        let line = format!("{}: {}{} °C", sensor_temp.name, pad, sensor_temp.temp);
        lines.push_back(colorize_from_temp(line, sensor_temp.temp, sensor_temp.sensor_type));
    }

    lines
}


#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_output_temps() {
        assert_eq!(output_temps(TempDeque::from(vec![SensorTemp{name: "sensor1".to_string(),
                                                                sensor_type: SensorType::CPU,
                                                                temp: 95},
                                                     SensorTemp{name: "sensor222222222".to_string(),
                                                                sensor_type: SensorType::DRIVE,
                                                                temp: 40},
                                                     SensorTemp{name: "sensor333".to_string(),
                                                                sensor_type: SensorType::OTHER,
                                                                temp: 50}])),
                   ["\u{1b}[31msensor1:         95 °C\u{1b}[0m",
                    "sensor222222222: 40 °C",
                    "\u{1b}[33msensor333:       50 °C\u{1b}[0m"]);
    }

    #[test]
    fn test_colorize_from_temp() {
        assert_eq!(colorize_from_temp("hey".to_string(), 59, SensorType::CPU), "hey");
        assert_eq!(colorize_from_temp("hey".to_string(), 60, SensorType::CPU), "\u{1b}[33mhey\u{1b}[0m");
        assert_eq!(colorize_from_temp("hey".to_string(), 61, SensorType::CPU), "\u{1b}[33mhey\u{1b}[0m");

        assert_eq!(colorize_from_temp("hey".to_string(), 74, SensorType::CPU), "\u{1b}[33mhey\u{1b}[0m");
        assert_eq!(colorize_from_temp("hey".to_string(), 75, SensorType::CPU), "\u{1b}[31mhey\u{1b}[0m");
        assert_eq!(colorize_from_temp("hey".to_string(), 76, SensorType::CPU), "\u{1b}[31mhey\u{1b}[0m");

        assert_eq!(colorize_from_temp("hey".to_string(), 44, SensorType::DRIVE), "hey");
        assert_eq!(colorize_from_temp("hey".to_string(), 45, SensorType::DRIVE), "\u{1b}[33mhey\u{1b}[0m");
        assert_eq!(colorize_from_temp("hey".to_string(), 46, SensorType::DRIVE), "\u{1b}[33mhey\u{1b}[0m");

        assert_eq!(colorize_from_temp("hey".to_string(), 54, SensorType::DRIVE), "\u{1b}[33mhey\u{1b}[0m");
        assert_eq!(colorize_from_temp("hey".to_string(), 55, SensorType::DRIVE), "\u{1b}[31mhey\u{1b}[0m");
        assert_eq!(colorize_from_temp("hey".to_string(), 56, SensorType::DRIVE), "\u{1b}[31mhey\u{1b}[0m");

        assert_eq!(colorize_from_temp("hey".to_string(), 49, SensorType::OTHER), "hey");
        assert_eq!(colorize_from_temp("hey".to_string(), 50, SensorType::OTHER), "\u{1b}[33mhey\u{1b}[0m");
        assert_eq!(colorize_from_temp("hey".to_string(), 51, SensorType::OTHER), "\u{1b}[33mhey\u{1b}[0m");

        assert_eq!(colorize_from_temp("hey".to_string(), 59, SensorType::OTHER), "\u{1b}[33mhey\u{1b}[0m");
        assert_eq!(colorize_from_temp("hey".to_string(), 60, SensorType::OTHER), "\u{1b}[31mhey\u{1b}[0m");
        assert_eq!(colorize_from_temp("hey".to_string(), 61, SensorType::OTHER), "\u{1b}[31mhey\u{1b}[0m");
    }
}
