use std::collections::VecDeque;
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;

use clap::{App, Arg};

mod fs;
mod load;
mod mem;
mod systemd;
mod temp;

/// Parsed command line arguments
struct CLArgs {
    /// Maximum terminal columns to use
    term_columns: usize,
}

/// Default terminal column count (width)
const DEFAULT_TERM_COLUMNS: usize = 80;

/// Output section header to stdout
fn output_title(title: &str, loading: bool, columns: usize) {
    println!("\n{:─^width$}", format!(" {} ", title), width = columns);
    if loading {
        print!("Loading...\r");
    }
}

/// Output lines to stdout
fn output_lines(lines: VecDeque<String>) {
    for line in lines {
        println!("{}", line);
    }
}

fn parse_cl_args() -> CLArgs {
    let default_term_columns_string = DEFAULT_TERM_COLUMNS.to_string();

    // clap arg matching
    let matches = App::new("motd")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Show dynamic summary of system information")
        .author("desbma")
        .arg(
            Arg::with_name("COLUMNS")
                .short("c")
                .long("columns")
                .takes_value(true)
                .allow_hyphen_values(true)
                .default_value(&default_term_columns_string)
                .help("Maximum terminal columns to use. Set to 0 to autotetect."),
        )
        .get_matches();

    // "post clap" parsing
    CLArgs {
        term_columns: match usize::from_str(matches.value_of("COLUMNS").unwrap())
            .expect("invalid columns value")
        {
            // Autodetect
            0 => {
                termsize::get()
                    // Detection failed, fallback to default
                    .unwrap_or(termsize::Size {
                        rows: 0,
                        cols: DEFAULT_TERM_COLUMNS as u16,
                    })
                    .cols as usize
            }
            // Passthrough
            v => v,
        },
    }
}

fn main() {
    let cl_args = parse_cl_args();

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

    output_title("Load", false, cl_args.term_columns);

    // Get load info
    let load_info = load::get_load_info();

    // Output load info
    let lines = load::output_load_info(load_info, 0);
    output_lines(lines);

    output_title("Memory usage", false, cl_args.term_columns);

    let mut mem_info = mem::MemInfo::new();

    // Get all memory usage info
    mem::get_mem_info(&mut mem_info);

    // Output memory usage
    let lines = mem::output_mem(&mem_info, cl_args.term_columns);
    output_lines(lines);

    // Output swap usage
    let lines = mem::output_swap(&mem_info, cl_args.term_columns);
    if !lines.is_empty() {
        output_title("Swap", false, cl_args.term_columns);

        output_lines(lines);
    }

    output_title("Filesystem usage", false, cl_args.term_columns);

    // Get filesystem info
    let fs_info = fs::get_fs_info();

    // Output filesystem info
    let lines = fs::output_fs_info(fs_info, cl_args.term_columns);
    output_lines(lines);

    output_title("Hardware temperatures", true, cl_args.term_columns);

    // Output temps
    temps = temps_rx.recv().unwrap();
    let lines = temp::output_temps(temps);
    output_lines(lines);

    // Get failed units
    for systemd_mode in &[systemd::SystemdMode::System, systemd::SystemdMode::User] {
        let failed_units = units_rx.recv().unwrap();
        if !failed_units.is_empty() {
            output_title(
                &format!(
                    "Systemd failed units ({})",
                    match systemd_mode {
                        systemd::SystemdMode::System => "system",
                        systemd::SystemdMode::User => "user",
                    }
                ),
                true,
                cl_args.term_columns,
            );

            // Output them
            let lines = systemd::output_failed_units(failed_units);
            output_lines(lines);
        }
    }
}
