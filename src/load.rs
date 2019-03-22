use std::collections::VecDeque;
use std::fs::File;
use std::io::prelude::*;
use std::str::FromStr;

use ansi_term::Colour::*;


/// Names of failed Systemd units
pub struct LoadInfo {
    load_avg_1m: f32,
    load_avg_5m: f32,
    load_avg_15m: f32,
    task_count: u32,
}


/// Fetch load information from /proc/loadavg
pub fn get_load_info() -> LoadInfo {
    let mut load_info = LoadInfo{load_avg_1m: 0.0,
                                 load_avg_5m: 0.0,
                                 load_avg_15m: 0.0,
                                 task_count: 0};

    let mut file = File::open("/proc/loadavg").unwrap();
    let mut line = String::new();
    file.read_to_string(&mut line).unwrap();

    let mut tokens_it = line.split(' ');
    load_info.load_avg_1m = f32::from_str(tokens_it.next().unwrap()).unwrap();
    load_info.load_avg_5m = f32::from_str(tokens_it.next().unwrap()).unwrap();
    load_info.load_avg_15m = f32::from_str(tokens_it.next().unwrap()).unwrap();

    load_info.task_count = u32::from_str(tokens_it.next().unwrap().split('/').skip(1).next().unwrap()).unwrap();

    load_info
}


/// Colorize load string
fn colorize_load(load: f32, cpu_count: usize) -> String  {
    if load >= cpu_count as f32 {
        Red.paint(load.to_string()).to_string()
    }
    else if load >= cpu_count as f32 * 0.8 {
        Yellow.paint(load.to_string()).to_string()
    }
    else {
        load.to_string()
    }
}


/// Output load information
pub fn output_load_info(load_info: LoadInfo) -> VecDeque<String> {
    let mut lines: VecDeque<String> = VecDeque::new();

    let cpu_count = num_cpus::get();
    lines.push_back(format!("Load avg 1min: {}, 5 min: {}, 15 min: {}",
                            colorize_load(load_info.load_avg_1m, cpu_count),
                            colorize_load(load_info.load_avg_5m, cpu_count),
                            colorize_load(load_info.load_avg_15m, cpu_count)));
    lines.push_back(format!("Tasks: {}", load_info.task_count));

    lines
}
