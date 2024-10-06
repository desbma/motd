use std::{
    fmt,
    io::BufRead,
    process::{Command, Stdio},
    thread,
};

use ansi_term::Colour::Red;

use crate::module::ModuleData;

/// Names of failed Systemd units
#[derive(Debug)]
pub(crate) struct FailedUnits {
    system: Vec<String>,
    user: Vec<String>,
}

/// Systemd running mode
enum SystemdMode {
    System,
    User,
}

/// Get name of Systemd units in failed state
pub(crate) fn fetch() -> anyhow::Result<ModuleData> {
    let system_fut = thread::spawn(|| fetch_mode(SystemdMode::System));
    let user = fetch_mode(SystemdMode::User)?;

    Ok(ModuleData::Systemd(FailedUnits {
        system: system_fut
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {:?}", e))??,
        user,
    }))
}

/// Get name of Systemd units in failed state
#[expect(clippy::needless_pass_by_value)]
fn fetch_mode(mode: SystemdMode) -> anyhow::Result<Vec<String>> {
    let mut args = match mode {
        SystemdMode::System => vec![],
        SystemdMode::User => vec!["--user"],
    };
    args.extend(&["--no-legend", "--plain", "--failed"]);
    let output = Command::new("systemctl")
        .args(&args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;
    anyhow::ensure!(output.status.success(), "systemctl failed");

    let mut units = Vec::new();
    for line in output.stdout.lines() {
        units.push(
            line?
                .trim_start()
                .split(' ')
                .next()
                .ok_or_else(|| anyhow::anyhow!("Failed to parse systemctl output"))?
                .to_owned(),
        );
    }

    Ok(units)
}

impl fmt::Display for FailedUnits {
    /// Output names of Systemd units in failed state
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !self.system.is_empty() {
            writeln!(f, "System:")?;
        }
        for u in &self.system {
            writeln!(f, "{}", Red.paint(u))?;
        }
        if !self.user.is_empty() {
            writeln!(f, "User:")?;
        }
        for u in &self.user {
            writeln!(f, "{}", Red.paint(u))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_failed_units() {
        assert_eq!(
            format!(
                "{}",
                FailedUnits {
                    system: vec!["foo.service".to_owned(), "bar.timer".to_owned()],
                    user: vec![]
                }
            ),
            "System:\n\u{1b}[31mfoo.service\u{1b}[0m\n\u{1b}[31mbar.timer\u{1b}[0m\n"
        );
        assert_eq!(
            format!(
                "{}",
                FailedUnits {
                    system: vec![],
                    user: vec!["foo.service".to_owned(), "bar.timer".to_owned()]
                }
            ),
            "User:\n\u{1b}[31mfoo.service\u{1b}[0m\n\u{1b}[31mbar.timer\u{1b}[0m\n"
        );
        assert_eq!(
            format!(
                "{}",
                FailedUnits {
                    system: vec!["foo.service".to_owned(), "bar.timer".to_owned()],
                    user: vec!["foo2.service".to_owned()]
                }
            ),
            "System:\n\u{1b}[31mfoo.service\u{1b}[0m\n\u{1b}[31mbar.timer\u{1b}[0m\nUser:\n\u{1b}[31mfoo2.service\u{1b}[0m\n"
        );
        assert_eq!(
            format!(
                "{}",
                FailedUnits {
                    system: vec![],
                    user: vec![]
                }
            ),
            ""
        );
    }
}
