use std::thread;

mod mem;
mod systemd;
mod temp;


/// Terminal column count (width)
const TERM_COLUMNS : usize = 80;  // TODO Get this dynamically?


fn output_title(title: &str) {
    println!("\n{:─^width$}",
             format!(" {} ", title),
             width=TERM_COLUMNS);
}


fn main() {
    if cfg!(feature = "worker_thread") {
        // Fetch systemd failed units in a background thread
        let mut failed_units = systemd::FailedUnits::new();
        let systemd_worker_thread = thread::Builder::new().name("systemd_worker".to_string()).spawn(move || {
            // Get systemd failed units
            systemd::get_failed_units(&mut failed_units);

            failed_units
        }).unwrap();

        // Fetch temps in a background thread
        let mut temps = temp::TempDeque::new();
        let temp_worker_thread = thread::Builder::new().name("temp_worker".to_string()).spawn(move || {
            // Get temps
            temp::get_hwmon_temps(&mut temps);
            temp::get_drive_temps(&mut temps);

            temps
        }).unwrap();


        output_title("Memory usage");

        let mut mem_info = mem::MemInfo::new();

        // Get all memory usage info
        mem::get_mem_info(&mut mem_info);

        // Output memory usage
        mem::output_mem(mem_info, TERM_COLUMNS);


        output_title("Hardware temperatures");

        // Output temps
        temps = temp_worker_thread.join().unwrap();
        temp::output_temps(temps);

        // Get failed units
        failed_units = systemd_worker_thread.join().unwrap();
        if !failed_units.is_empty() {
            output_title("Systemd failed units");

            // Output them
            systemd::output_failed_units(failed_units);
        }
    }
    else {
        output_title("Memory usage");

        let mut mem_info = mem::MemInfo::new();

        // Get all memory usage info
        mem::get_mem_info(&mut mem_info);

        // Output memory usage
        mem::output_mem(mem_info, TERM_COLUMNS);


        output_title("Hardware temperatures");

        // Get temps
        let mut temps = temp::TempDeque::new();
        temp::get_hwmon_temps(&mut temps);
        temp::get_drive_temps(&mut temps);

        // Output temps
        temp::output_temps(temps);


        // Get systemd failed units
        let mut failed_units = systemd::FailedUnits::new();
        systemd::get_failed_units(&mut failed_units);

        if !failed_units.is_empty() {
            output_title("Systemd failed units");

            // Output them
            systemd::output_failed_units(failed_units);
        }
    }
}
