use std::collections::VecDeque;
use std::io::BufRead;
use std::process::{Command, Stdio};

use ansi_term::Colour::*;


/// Names of failed Systemd units
pub type FailedUnits = VecDeque<String>;


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


pub fn output_failed_units(units: FailedUnits) {
    for unit in units {
        println!("{}", Red.paint(unit));
    }
}
