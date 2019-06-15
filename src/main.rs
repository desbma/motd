use std::collections::VecDeque;
use std::iter::Iterator;
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;

use clap::{App, Arg};

mod fs;
mod load;
mod mem;
mod systemd;
mod temp;

/// Output section
enum Section {
    Load,
    Mem,
    Swap,
    FS,
    Temps,
    SDFailedUnits,
}

/// Parsed command line arguments
struct CLArgs {
    /// Maximum terminal columns to use
    term_columns: usize,

    /// Sections to display
    #[allow(dead_code)]
    sections: Vec<Section>,
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

fn section_to_letter(section: &Section) -> String {
    match section {
        Section::Load => "l".to_string(),
        Section::Mem => "m".to_string(),
        Section::Swap => "s".to_string(),
        Section::FS => "f".to_string(),
        Section::Temps => "t".to_string(),
        Section::SDFailedUnits => "u".to_string(),
    }
}

fn letter_to_section(letter: &str) -> Section {
    match letter {
        "l" => Section::Load,
        "m" => Section::Mem,
        "s" => Section::Swap,
        "f" => Section::FS,
        "t" => Section::Temps,
        "u" => Section::SDFailedUnits,
        _ => panic!(), // validated by clap
    }
}

/// Validate a usize integer string for Clap usage
fn validator_usize(s: String) -> Result<(), String> {
    match usize::from_str(&s) {
        Ok(_) => Ok(()),
        Err(_) => Err("Not a valid positive integer value".to_string()),
    }
}

/// Parse and validate command line arguments
fn parse_cl_args() -> CLArgs {
    // Default values
    let default_term_columns_string = DEFAULT_TERM_COLUMNS.to_string();
    let sections_string: Vec<String> = vec![
        Section::Load,
        Section::Mem,
        Section::Swap,
        Section::FS,
        Section::Temps,
        Section::SDFailedUnits,
    ]
    .iter()
    .map(|s| section_to_letter(s))
    .collect();
    let default_sections_string = sections_string.join(",");
    let sections_str: Vec<&str> = sections_string.iter().map(String::as_str).collect();

    // Clap arg matching
    let matches = App::new("motd")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Show dynamic summary of system information")
        .author("desbma")
        .arg(
            Arg::with_name("SECTIONS")
                .short("s")
                .long("sections")
                .takes_value(true)
                .multiple(true)
                .use_delimiter(true)
                .default_value(&default_sections_string)
                .possible_values(&sections_str)
                .help(
                    "Sections to display. \
                     l: Systemd load. \
                     m: Memory. \
                     s: Swap.\
                     f: Filesystem. \
                     t: Hardware temperatures. \
                     u: Systemd failed units.",
                ),
        )
        .arg(
            Arg::with_name("COLUMNS")
                .short("c")
                .long("columns")
                .takes_value(true)
                .allow_hyphen_values(true)
                .validator(validator_usize)
                .default_value(&default_term_columns_string)
                .help("Maximum terminal columns to use. Set to 0 to autotetect."),
        )
        .get_matches();

    // Post Clap parsing
    let sections = matches
        .values_of("SECTIONS")
        .unwrap()
        .map(|s| letter_to_section(s))
        .collect();
    let term_columns = match usize::from_str(matches.value_of("COLUMNS").unwrap()).unwrap() {
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
    };

    CLArgs {
        sections,
        term_columns,
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
