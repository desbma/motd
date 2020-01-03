use std::collections::VecDeque;
use std::error;
use std::io::BufRead;
use std::process::{Command, Stdio};

use ansi_term::Colour::*;
use simple_error::SimpleError;

/// Names of failed Systemd units
type FailedUnits = VecDeque<String>;

/// Systemd running mode
pub enum SystemdMode {
    System,
    User,
}

/// Get name of Systemd units in failed state
pub fn get_failed_units(mode: &SystemdMode) -> Result<FailedUnits, Box<dyn error::Error>> {
    let mut units: FailedUnits = FailedUnits::new();

    let mut args = match mode {
        SystemdMode::System => vec![],
        SystemdMode::User => vec!["--user"],
    };
    args.extend(&["--no-legend", "--failed"]);
    let output = Command::new("systemctl")
        .args(&args)
        .stderr(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(Box::new(SimpleError::new("systemctl failed")));
    }
    for line in output.stdout.lines() {
        units.push_back(
            line?
                .split(' ')
                .next()
                .ok_or_else(|| SimpleError::new("Failed to parse systemctl output"))?
                .to_string(),
        );
    }

    Ok(units)
}

/// Output names of Systemd units in failed state
pub fn output_failed_units(units: FailedUnits) -> VecDeque<String> {
    units.iter().map(|u| Red.paint(u).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_failed_units() {
        assert_eq!(
            output_failed_units(FailedUnits::from(vec![
                "foo.service".to_string(),
                "bar.timer".to_string()
            ])),
            [
                "\u{1b}[31mfoo.service\u{1b}[0m",
                "\u{1b}[31mbar.timer\u{1b}[0m"
            ]
        );
    }
}
