use std::{fmt, fs, str::FromStr, sync::atomic::Ordering};

use ansi_term::Colour::{Red, Yellow};

use crate::module::{ModuleData, CPU_COUNT};

/// Names of failed Systemd units
#[derive(Debug)]
pub(crate) struct LoadInfo {
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
#[expect(clippy::similar_names)]
pub(crate) fn fetch() -> anyhow::Result<ModuleData> {
    let line = fs::read_to_string("/proc/loadavg")?;

    let mut tokens_it = line.split(' ');
    let load_avg_1m = f32::from_str(
        tokens_it
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to parse load average 1m"))?,
    )?;
    let load_avg_5m = f32::from_str(
        tokens_it
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to parse load average 5m"))?,
    )?;
    let load_avg_15m = f32::from_str(
        tokens_it
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to parse load average 15m"))?,
    )?;

    let task_count = u32::from_str(
        tokens_it
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to parse task count"))?
            .split('/')
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse task count"))?,
    )?;

    Ok(ModuleData::Load(LoadInfo {
        load_avg_1m,
        load_avg_5m,
        load_avg_15m,
        task_count,
    }))
}

impl fmt::Display for LoadInfo {
    /// Output load information
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cpu_count = CPU_COUNT.load(Ordering::SeqCst);
        writeln!(
            f,
            "Load avg 1min: {}, 5 min: {}, 15 min: {}",
            colorize_load(self.load_avg_1m, cpu_count),
            colorize_load(self.load_avg_5m, cpu_count),
            colorize_load(self.load_avg_15m, cpu_count)
        )?;
        writeln!(f, "Tasks: {}", self.task_count)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_load_info() {
        CPU_COUNT.store(3, Ordering::SeqCst);
        assert_eq!(
            format!(
                "{}",
                LoadInfo {
                    load_avg_1m: 1.1,
                    load_avg_5m: 2.9,
                    load_avg_15m: 3.1,
                    task_count: 12345,
                },
            ),
            "Load avg 1min: 1.1, 5 min: \u{1b}[33m2.9\u{1b}[0m, 15 min: \u{1b}[31m3.1\u{1b}[0m\nTasks: 12345\n"
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
