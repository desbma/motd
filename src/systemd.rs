use std::collections::VecDeque;
use std::io::BufRead;
use std::process::{Command, Stdio};

use ansi_term::Colour::*;


/// Names of failed Systemd units
pub type FailedUnits = VecDeque<String>;


/// Get name of Systemd units in failed state
pub fn get_failed_units(units: &mut FailedUnits) {
    let output = Command::new("systemctl")
                          .args(&["--no-legend", "--failed"])
                          .stderr(Stdio::null())
                          .output();
    let stdout = match output {
        Ok(o) => o.stdout,
        Err(_e) => return,
    };
    for line in stdout.lines() {
        let line = line.unwrap();
        let mut tokens_it = line.split(' ');
        let unit = tokens_it.next().unwrap().to_string();
        units.push_back(unit);
    }
}


/// Output names of Systemd units in failed state
pub fn output_failed_units(units: FailedUnits) -> VecDeque<String> {
    let mut lines: VecDeque<String> = VecDeque::new();

    for unit in units {
        lines.push_back(Red.paint(unit).to_string());
    }

    lines
}


#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_output_failed_units() {
        assert_eq!(output_failed_units(FailedUnits::from(vec!["foo.service".to_string(),
                                                              "bar.timer".to_string()])),
                   ["\u{1b}[31mfoo.service\u{1b}[0m",
                    "\u{1b}[31mbar.timer\u{1b}[0m"]);
    }
}
