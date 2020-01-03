use std::collections::VecDeque;
use std::error;
use std::fs::File;
use std::io::prelude::*;
use std::str::FromStr;

use ansi_term::Colour::*;

/// Names of failed Systemd units
pub struct LoadInfo {
    /// Load average 1 minute
    load_avg_1m: f32,
    /// Load average 5 minutes
    load_avg_5m: f32,
    /// Load average 15 minutes
    load_avg_15m: f32,
    /// Total task count
    task_count: u32,
}

/// Fetch load information from /proc/loadavg
pub fn get_load_info() -> Result<LoadInfo, Box<dyn error::Error>> {
    let mut load_info = LoadInfo {
        load_avg_1m: 0.0,
        load_avg_5m: 0.0,
        load_avg_15m: 0.0,
        task_count: 0,
    };

    let mut file = File::open("/proc/loadavg").unwrap();
    let mut line = String::new();
    file.read_to_string(&mut line).unwrap();

    let mut tokens_it = line.split(' ');
    load_info.load_avg_1m = f32::from_str(tokens_it.next().unwrap()).unwrap();
    load_info.load_avg_5m = f32::from_str(tokens_it.next().unwrap()).unwrap();
    load_info.load_avg_15m = f32::from_str(tokens_it.next().unwrap()).unwrap();

    load_info.task_count =
        u32::from_str(tokens_it.next().unwrap().split('/').nth(1).unwrap()).unwrap();

    Ok(load_info)
}

/// Colorize load string
fn colorize_load(load: f32, cpu_count: usize) -> String {
    if load >= cpu_count as f32 {
        Red.paint(load.to_string()).to_string()
    } else if load >= cpu_count as f32 * 0.8 {
        Yellow.paint(load.to_string()).to_string()
    } else {
        load.to_string()
    }
}

/// Output load information
pub fn output_load_info(load_info: LoadInfo, default_cpu_count: usize) -> VecDeque<String> {
    let mut lines: VecDeque<String> = VecDeque::new();

    let cpu_count = if default_cpu_count == 0 {
        num_cpus::get()
    } else {
        default_cpu_count
    };
    lines.push_back(format!(
        "Load avg 1min: {}, 5 min: {}, 15 min: {}",
        colorize_load(load_info.load_avg_1m, cpu_count),
        colorize_load(load_info.load_avg_5m, cpu_count),
        colorize_load(load_info.load_avg_15m, cpu_count)
    ));
    lines.push_back(format!("Tasks: {}", load_info.task_count));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_load_info() {
        assert_eq!(
            output_load_info(
                LoadInfo {
                    load_avg_1m: 1.1,
                    load_avg_5m: 2.9,
                    load_avg_15m: 3.1,
                    task_count: 12345
                },
                3
            ),
            [
                "Load avg 1min: 1.1, 5 min: \u{1b}[33m2.9\u{1b}[0m, 15 min: \u{1b}[31m3.1\u{1b}[0m",
                "Tasks: 12345"
            ]
        );
    }

    #[test]
    fn test_colorize_load() {
        assert_eq!(colorize_load(7.9, 10), "7.9");
        assert_eq!(colorize_load(8.0, 10), "\u{1b}[33m8\u{1b}[0m");
        assert_eq!(colorize_load(8.1, 10), "\u{1b}[33m8.1\u{1b}[0m");
        assert_eq!(colorize_load(9.9, 10), "\u{1b}[33m9.9\u{1b}[0m");
        assert_eq!(colorize_load(10.0, 10), "\u{1b}[31m10\u{1b}[0m");
        assert_eq!(colorize_load(10.1, 10), "\u{1b}[31m10.1\u{1b}[0m");
    }
}
