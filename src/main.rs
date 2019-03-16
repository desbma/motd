use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use std::process;
use std::str::FromStr;

use glob::glob;


enum SensorType {
    DRIVE,
    OTHER,
}


struct SensorTemp {
    name: String,
    sensor_type: SensorType,
    temp: u32,
}


type TempDeque = VecDeque<SensorTemp>;


fn get_cpu_temps(temps: &mut TempDeque) {
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

            // Read temp
            let input_temp_filepath = format!("{}_input", input_label_filepath[..input_label_filepath.len() - 6].to_owned());  // TODO optimize this?
            let mut input_temp_file = File::open(input_temp_filepath).unwrap();
            let mut temp_str = String::new();
            input_temp_file.read_to_string(&mut temp_str).unwrap();
            let temp_val = temp_str.trim_end().parse::<u32>().unwrap() / 1000;

            // Store temp
            let sensor_temp = SensorTemp {name: label,
                                          sensor_type: SensorType::OTHER,
                                          temp: temp_val};
            temps.push_back(sensor_temp);
        }
    }
}


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


fn get_drive_temps(temps: &mut TempDeque) {
    // Connect
    let mut stream = match TcpStream::connect("127.0.0.1:7634") {  // TODO port const
        Ok(s) => s,
        Err(_e) => process::exit(0),  // TODO use EXIT_SUCCESS
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


fn output_temps(temps: TempDeque) {
    let mut max_name_len = 0;
    for sensor_temp in &temps {
        let name_len = sensor_temp.name.len();
        if name_len > max_name_len {
            max_name_len = name_len;
        }
    }
    for sensor_temp in temps {
        let pad = " ".repeat(max_name_len - sensor_temp.name.len());
        // TODO coloring
        println!("{}: {}{} °C", sensor_temp.name, pad, sensor_temp.temp);
    }
}


fn main() {
    let mut temps: TempDeque = TempDeque::new();

    // CPU temps
    get_cpu_temps(&mut temps);

    // Drive temps
    get_drive_temps(&mut temps);

    // Output
    output_temps(temps);
}
