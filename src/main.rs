use std::collections::VecDeque;
use std::sync::mpsc;
use std::thread;

mod fs;
mod load;
mod mem;
mod systemd;
mod temp;

/// Terminal column count (width)
const TERM_COLUMNS: usize = 80; // TODO Get this dynamically?

/// Output section header to stdout
fn output_title(title: &str) {
    println!(
        "\n{:─^width$}",
        format!(" {} ", title),
        width = TERM_COLUMNS
    );
}

/// Output lines to stdout
fn output_lines(lines: VecDeque<String>) {
    for line in lines {
        println!("{}", line);
    }
}

fn main() {
    // Fetch systemd failed units in a background thread
    let (units_tx, units_rx) = mpsc::channel();
    thread::Builder::new()
        .name("systemd_worker".to_string())
        .spawn(move || {
            for systemd_mode in &[systemd::SystemdMode::System, systemd::SystemdMode::User] {
                // Get systemd failed units
                let mut failed_units = systemd::FailedUnits::new();
                systemd::get_failed_units(&mut failed_units, systemd_mode);
                units_tx.send(failed_units).unwrap();
            }
        })
        .unwrap();

    // Fetch temps in a background thread
    let (temps_tx, temps_rx) = mpsc::channel();
    let mut temps = temp::TempDeque::new();
    thread::Builder::new()
        .name("temp_worker".to_string())
        .spawn(move || {
            // Get temps
            temp::get_hwmon_temps(&mut temps);
            temp::get_drive_temps(&mut temps);
            temps_tx.send(temps).unwrap();
        })
        .unwrap();

    output_title("Load");

    // Get load info
    let load_info = load::get_load_info();

    // Output load info
    let lines = load::output_load_info(load_info, 0);
    output_lines(lines);

    output_title("Memory usage");

    let mut mem_info = mem::MemInfo::new();

    // Get all memory usage info
    mem::get_mem_info(&mut mem_info);

    // Output memory usage
    let lines = mem::output_mem(&mem_info, TERM_COLUMNS);
    output_lines(lines);

    // Output swap usage
    let lines = mem::output_swap(&mem_info, TERM_COLUMNS);
    if !lines.is_empty() {
        output_title("Swap");

        output_lines(lines);
    }

    output_title("Filesystem usage");

    // Get filesystem info
    let fs_info = fs::get_fs_info();

    // Output filesystem info
    let lines = fs::output_fs_info(fs_info, TERM_COLUMNS);
    output_lines(lines);

    output_title("Hardware temperatures");

    // Output temps
    temps = temps_rx.recv().unwrap();
    let lines = temp::output_temps(temps);
    output_lines(lines);

    // Get failed units
    for systemd_mode in &[systemd::SystemdMode::System, systemd::SystemdMode::User] {
        let failed_units = units_rx.recv().unwrap();
        if !failed_units.is_empty() {
            output_title(&format!(
                "Systemd failed units ({})",
                match systemd_mode {
                    systemd::SystemdMode::System => "system",
                    systemd::SystemdMode::User => "user",
                }
            ));

            // Output them
            let lines = systemd::output_failed_units(failed_units);
            output_lines(lines);
        }
    }
}
