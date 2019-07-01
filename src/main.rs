use std::collections::VecDeque;
use std::io;
use std::io::prelude::*;
use std::iter::Iterator;
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;

use clap::{App, Arg};
use itertools::Itertools;

mod fs;
mod load;
mod mem;
mod systemd;
mod temp;

/// Output section
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
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
    sections: Vec<Section>,

    /// Whether or not to display each section title
    show_section_titles: bool,
}

/// Default terminal column count (width)
const DEFAULT_TERM_COLUMNS: usize = 80;

/// Output section header to stdout
fn output_title(title: &str, columns: usize) {
    println!("\n{:─^width$}", format!(" {} ", title), width = columns);
}

/// Output lines to stdout
fn output_lines(lines: VecDeque<String>) {
    for line in lines {
        println!("{}", line);
    }
}

/// Output section title and lines
fn output_section(
    title: &str,
    lines: Option<VecDeque<String>>,
    lines_rx: Option<&mpsc::Receiver<VecDeque<String>>>,
    show_title: bool,
    columns: usize,
) {
    if lines_rx.is_some() {
        print!("Loading...\r");
        io::stdout().flush().unwrap();
    }

    let lines = match lines_rx {
        Some(chan) => chan.recv().unwrap(),
        None => lines.unwrap(),
    };

    if lines_rx.is_some() {
        print!("          \r");
        io::stdout().flush().unwrap();
    }

    if !lines.is_empty() {
        if show_title {
            output_title(title, columns);
        }

        output_lines(lines);
    }
}

/// Get Section from letter
fn section_to_letter(section: Section) -> String {
    match section {
        Section::Load => "l".to_string(),
        Section::Mem => "m".to_string(),
        Section::Swap => "s".to_string(),
        Section::FS => "f".to_string(),
        Section::Temps => "t".to_string(),
        Section::SDFailedUnits => "u".to_string(),
    }
}

/// Get letter from Section
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
    .map(|s| section_to_letter(*s))
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
            Arg::with_name("NO_TITLES")
                .short("n")
                .long("no-titles")
                .help("Do not display section titles."),
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
        .unique()
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
    let show_section_titles = !matches.is_present("NO_TITLES");

    CLArgs {
        sections,
        term_columns,
        show_section_titles,
    }
}

fn main() {
    let cl_args = parse_cl_args();

    // Fetch systemd failed units in a background thread
    let (unit_lines_tx, unit_lines_rx) = mpsc::channel();
    thread::Builder::new()
        .name("systemd_worker".to_string())
        .spawn(move || {
            for systemd_mode in &[systemd::SystemdMode::System, systemd::SystemdMode::User] {
                // Get systemd failed units
                let mut failed_units = systemd::FailedUnits::new();
                systemd::get_failed_units(&mut failed_units, systemd_mode);

                // Format them to lines
                let lines = systemd::output_failed_units(failed_units);

                // Send them to main thread
                unit_lines_tx.send(lines).unwrap();
            }
        })
        .unwrap();

    // Fetch temps in a background thread
    let (temp_lines_tx, temp_lines_rx) = mpsc::channel();
    thread::Builder::new()
        .name("temp_worker".to_string())
        .spawn(move || {
            // Get temps
            let mut temps = temp::TempDeque::new();
            temp::get_hwmon_temps(&mut temps);
            temp::get_drive_temps(&mut temps);

            // Format them to lines
            let lines = temp::output_temps(temps);

            // Send them to main thread
            temp_lines_tx.send(lines).unwrap();
        })
        .unwrap();

    let mut mem_info: Option<mem::MemInfo> = None;

    let last_section = *cl_args.sections.last().unwrap();

    for section in cl_args.sections {
        match section {
            Section::Load => {
                // Load info
                let load_info = load::get_load_info();
                let lines = load::output_load_info(load_info, 0);
                output_section(
                    "Load",
                    Some(lines),
                    None,
                    cl_args.show_section_titles,
                    cl_args.term_columns,
                );
            }

            Section::Mem => {
                // Memory usage
                if (&mem_info).is_none() {
                    mem_info = Some(mem::get_mem_info());
                }
                let lines = mem::output_mem(&(mem_info.as_ref()).unwrap(), cl_args.term_columns);
                output_section(
                    "Memory usage",
                    Some(lines),
                    None,
                    cl_args.show_section_titles,
                    cl_args.term_columns,
                );
            }

            Section::Swap => {
                // Swap usage
                if (&mem_info).is_none() {
                    mem_info = Some(mem::get_mem_info());
                }
                let lines = mem::output_swap(&(mem_info.as_ref()).unwrap(), cl_args.term_columns);
                output_section(
                    "Swap",
                    Some(lines),
                    None,
                    cl_args.show_section_titles,
                    cl_args.term_columns,
                );
            }

            Section::FS => {
                // Filesystem info
                let fs_info = fs::get_fs_info();
                let lines = fs::output_fs_info(fs_info, cl_args.term_columns);
                output_section(
                    "Filesystem usage",
                    Some(lines),
                    None,
                    cl_args.show_section_titles,
                    cl_args.term_columns,
                );
            }

            Section::Temps => {
                // Temps
                output_section(
                    "Hardware temperatures",
                    None,
                    Some(&temp_lines_rx),
                    cl_args.show_section_titles,
                    cl_args.term_columns,
                );
            }

            Section::SDFailedUnits => {
                // Systemd failed units
                for systemd_mode in &[systemd::SystemdMode::System, systemd::SystemdMode::User] {
                    output_section(
                        &format!(
                            "Systemd failed units ({})",
                            match systemd_mode {
                                systemd::SystemdMode::System => "system",
                                systemd::SystemdMode::User => "user",
                            }
                        ),
                        None,
                        Some(&unit_lines_rx),
                        cl_args.show_section_titles,
                        cl_args.term_columns,
                    );
                }
            }
        }

        if !cl_args.show_section_titles && (section != last_section) {
            println!();
        }
    }
}
