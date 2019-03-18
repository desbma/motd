use std::collections::VecDeque;


/// Names of failed Systemd units
pub type FailedUnits = VecDeque<String>;


pub fn get_failed_units(units: &mut FailedUnits) {

}


pub fn output_failed_units(units: FailedUnits) {

}
