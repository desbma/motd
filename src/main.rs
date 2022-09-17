use std::cmp;
use std::io;
use std::io::prelude::*;
use std::iter::Iterator;
use std::str::FromStr;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;

use ansi_term::Colour::*;
use clap::{App, Arg};
use itertools::Itertools;

mod fmt;
mod fs;
mod load;
mod mem;
mod net;
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
    Network,
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

/// Fallback terminal column count (width), if it could not be detected
const FALLBACK_TERM_COLUMNS: usize = 80;

/// Output section header to stdout
fn output_title(title: &str, columns: usize, new_line: bool) {
    if new_line {
        println!();
    }
    println!("{:─^width$}", format!(" {} ", title), width = columns);
}

/// Output lines to stdout
fn output_lines(lines: &[String]) {
    for line in lines {
        println!("{}", line);
    }
}

/// Output section title and lines
fn output_section(
    title: &str,
    lines: Option<anyhow::Result<Vec<String>>>,
    lines_rx: Option<&mpsc::Receiver<anyhow::Result<Vec<String>>>>,
    show_title: bool,
    first_section: bool,
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

    match lines {
        Ok(lines) => {
            if !lines.is_empty() {
                if show_title {
                    output_title(title, columns, !first_section);
                } else if !first_section {
                    println!();
                }

                output_lines(&lines);
            }
        }
        Err(err) => {
            eprintln!(
                "{}",
                Red.paint(format!(
                    "Failed to get data for '{}' section: {}",
                    title, err
                ))
            );
        }
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
        Section::Network => "n".to_string(),
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
        "n" => Section::Network,
        "u" => Section::SDFailedUnits,
        _ => unreachable!(), // validated by clap
    }
}

/// Validate a isize integer string for Clap usage
fn validator_isize(s: &str) -> Result<(), String> {
    match isize::from_str(s) {
        Ok(_) => Ok(()),
        Err(_) => Err("Not a valid integer value".to_string()),
    }
}

/// Parse and validate command line arguments
fn parse_cl_args() -> CLArgs {
    // Default values
    let default_term_columns_string = format!("-{}", FALLBACK_TERM_COLUMNS);
    let sections_string: Vec<String> = vec![
        Section::Load,
        Section::Mem,
        Section::Swap,
        Section::FS,
        Section::Temps,
        Section::Network,
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
                .short('s')
                .long("sections")
                .takes_value(true)
                .multiple(true)
                .use_delimiter(true)
                .default_value(&default_sections_string)
                .possible_values(&sections_str)
                .help(
                    "Sections to display. \
                     l: System load. \
                     m: Memory. \
                     s: Swap.\
                     f: Filesystem usage. \
                     t: Hardware temperatures. \
                     n: Network interface stats. \
                     u: Systemd failed units."
                ),
        )
        .arg(
            Arg::with_name("NO_TITLES")
                .short('n')
                .long("no-titles")
                .help("Do not display section titles."),
        )
        .arg(
            Arg::with_name("COLUMNS")
                .short('c')
                .long("columns")
                .takes_value(true)
                .allow_hyphen_values(true)
                .validator(validator_isize)
                .default_value(&default_term_columns_string)
                .help("Maximum terminal columns to use. Set to 0 to autotetect. -X to use autodetected value or X, whichever is lower."),
        )
        .get_matches();

    // Post Clap parsing
    let sections = matches
        .values_of("SECTIONS")
        .unwrap()
        .map(letter_to_section)
        .unique()
        .collect();
    let term_columns: usize = match isize::from_str(matches.value_of("COLUMNS").unwrap()).unwrap() {
        0 => {
            // Autodetect
            termsize::get()
                // Detection failed, fallback to default
                .unwrap_or(termsize::Size {
                    rows: 0,
                    cols: FALLBACK_TERM_COLUMNS as u16,
                })
                .cols as usize
        }
        v if v < 0 => {
            // Autodetect with minimum
            cmp::min(
                -v as usize,
                termsize::get()
                    // Detection failed, fallback to default
                    .unwrap_or(termsize::Size {
                        rows: 0,
                        cols: FALLBACK_TERM_COLUMNS as u16,
                    })
                    .cols as usize,
            )
        }
        // Passthrough
        v => v as usize,
    };
    let show_section_titles = !matches.is_present("NO_TITLES");

    CLArgs {
        term_columns,
        sections,
        show_section_titles,
    }
}

#[allow(clippy::mutex_atomic)]
fn main() -> anyhow::Result<()> {
    let cl_args = parse_cl_args();

    // Fetch network stats in a background thread if needed
    let mut network_lines_rx: Option<mpsc::Receiver<anyhow::Result<Vec<String>>>> = None;
    let network_lines_needed_sync = Arc::new((Mutex::new(false), Condvar::new()));
    let network_lines_needed_sync2 = network_lines_needed_sync.clone();
    let (network_lines_needed_mutex, network_lines_needed_cv) = &*network_lines_needed_sync;
    if cl_args.sections.contains(&Section::Network)
        && (*cl_args.sections.first().unwrap() != Section::Network)
    {
        let (chan_tx, chan_rx) = mpsc::channel();
        network_lines_rx = Some(chan_rx);
        thread::Builder::new()
            .name("network_worker".to_string())
            .spawn(move || {
                // Get network stats
                let network_stats_sample = net::get_network_stats();

                let lines = match network_stats_sample {
                    Ok(mut network_stats_sample) => {
                        // Wait
                        let (network_lines_needed_mutex, network_lines_needed_cv) =
                            &*network_lines_needed_sync2;
                        let mut network_lines_needed = network_lines_needed_mutex.lock().unwrap();
                        while !*network_lines_needed {
                            network_lines_needed =
                                network_lines_needed_cv.wait(network_lines_needed).unwrap();
                        }

                        // Update stats
                        let network_stats = net::update_network_stats(&mut network_stats_sample);

                        // Format them to lines
                        network_stats.map(net::output_network_stats)
                    }
                    Err(e) => Err(e),
                };

                // Send them to main thread
                chan_tx.send(lines).unwrap();
            })
            .unwrap();
    }

    // Fetch systemd failed units in a background thread if needed
    let mut unit_lines_rx: Option<mpsc::Receiver<anyhow::Result<Vec<String>>>> = None;
    if cl_args.sections.contains(&Section::SDFailedUnits)
        && (*cl_args.sections.first().unwrap() != Section::SDFailedUnits)
    {
        let (chan_tx, chan_rx) = mpsc::channel();
        unit_lines_rx = Some(chan_rx);
        thread::Builder::new()
            .name("systemd_worker".to_string())
            .spawn(move || {
                for systemd_mode in &[systemd::SystemdMode::System, systemd::SystemdMode::User] {
                    // Get systemd failed units
                    let failed_units = systemd::get_failed_units(systemd_mode);

                    // Format them to lines
                    let lines = failed_units.map(systemd::output_failed_units);

                    // Send them to main thread
                    chan_tx.send(lines).unwrap();
                }
            })
            .unwrap();
    }

    // Fetch temps in a background thread if needed
    let mut temp_lines_rx: Option<mpsc::Receiver<anyhow::Result<Vec<String>>>> = None;
    if cl_args.sections.contains(&Section::Temps)
        && (*cl_args.sections.first().unwrap() != Section::Temps)
    {
        let (chan_tx, chan_rx) = mpsc::channel();
        temp_lines_rx = Some(chan_rx);
        thread::Builder::new()
            .name("temp_worker".to_string())
            .spawn(move || {
                // Get temps
                let temps = match temp::get_hwmon_temps() {
                    Ok(mut hwmon_temps) => match temp::get_drive_temps() {
                        Ok(drive_temps) => {
                            hwmon_temps.extend(drive_temps);
                            Ok(hwmon_temps)
                        }
                        Err(err) => Err(err),
                    },
                    Err(err) => Err(err),
                };

                // Format them to lines
                let lines = temps.map(temp::output_temps);

                // Send them to main thread
                chan_tx.send(lines).unwrap();
            })
            .unwrap();
    }

    let mut mem_info: Option<anyhow::Result<mem::MemInfo>> = None;

    let first_section = cl_args.sections.first().unwrap();

    for section in &cl_args.sections {
        match section {
            Section::Load => {
                // Load info
                let load_info = load::get_load_info();
                let lines = load_info.map(|l| load::output_load_info(l, 0));
                output_section(
                    "Load",
                    Some(lines),
                    None,
                    cl_args.show_section_titles,
                    section == first_section,
                    cl_args.term_columns,
                );
            }

            Section::Mem => {
                // Memory usage
                let lines: anyhow::Result<Vec<String>> =
                    match mem_info.get_or_insert_with(mem::get_mem_info) {
                        Ok(mi) => Ok(mem::output_mem(mi, cl_args.term_columns)),
                        Err(e) => anyhow::bail!("{}", e),
                    };
                output_section(
                    "Memory usage",
                    Some(lines),
                    None,
                    cl_args.show_section_titles,
                    section == first_section,
                    cl_args.term_columns,
                );
            }

            Section::Swap => {
                // Swap usage
                let lines: anyhow::Result<Vec<String>> =
                    match mem_info.get_or_insert_with(mem::get_mem_info) {
                        Ok(mi) => Ok(mem::output_swap(mi, cl_args.term_columns)),
                        Err(e) => anyhow::bail!("{}", e),
                    };
                output_section(
                    "Swap",
                    Some(lines),
                    None,
                    cl_args.show_section_titles,
                    section == first_section,
                    cl_args.term_columns,
                );
            }

            Section::FS => {
                // Filesystem info
                let fs_info = fs::get_fs_info();
                let lines = fs_info.map(|f| fs::output_fs_info(f, cl_args.term_columns));
                output_section(
                    "Filesystem usage",
                    Some(lines),
                    None,
                    cl_args.show_section_titles,
                    section == first_section,
                    cl_args.term_columns,
                );
            }

            Section::Temps => {
                // Temps
                let lines = match temp_lines_rx {
                    None => {
                        // Get temps
                        let mut temps = temp::get_hwmon_temps();
                        if let Ok(ref mut hwmon_temps) = temps {
                            match temp::get_drive_temps() {
                                Ok(drive_temps) => {
                                    hwmon_temps.extend(drive_temps);
                                }
                                Err(err) => {
                                    temps = Err(err);
                                }
                            }
                        }

                        // Format them to lines
                        Some(temps.map(temp::output_temps))
                    }
                    Some(_) => None,
                };
                output_section(
                    "Hardware temperatures",
                    lines,
                    temp_lines_rx.as_ref(),
                    cl_args.show_section_titles,
                    section == first_section,
                    cl_args.term_columns,
                );
            }

            Section::Network => {
                // Network stats
                let lines = match network_lines_rx {
                    None => {
                        // Get network stats
                        let network_stats_sample = net::get_network_stats();
                        let network_stats = match network_stats_sample {
                            Ok(mut network_stats_sample) => {
                                net::update_network_stats(&mut network_stats_sample)
                            }
                            Err(e) => Err(e),
                        };

                        // Format them to lines
                        Some(network_stats.map(net::output_network_stats))
                    }
                    Some(_) => {
                        // Signal the other side of the channel we need lines now
                        let mut network_lines_needed = network_lines_needed_mutex.lock().unwrap();
                        *network_lines_needed = true;
                        network_lines_needed_cv.notify_one();

                        None
                    }
                };

                output_section(
                    "Network",
                    lines,
                    network_lines_rx.as_ref(),
                    cl_args.show_section_titles,
                    section == first_section,
                    cl_args.term_columns,
                );
            }

            Section::SDFailedUnits => {
                // Systemd failed units
                for systemd_mode in &[systemd::SystemdMode::System, systemd::SystemdMode::User] {
                    let lines = match unit_lines_rx {
                        None => {
                            // Get systemd failed units
                            let failed_units = systemd::get_failed_units(systemd_mode);

                            // Format them to lines
                            Some(failed_units.map(systemd::output_failed_units))
                        }
                        Some(_) => None,
                    };
                    output_section(
                        &format!(
                            "Systemd failed units ({})",
                            match systemd_mode {
                                systemd::SystemdMode::System => "system",
                                systemd::SystemdMode::User => "user",
                            }
                        ),
                        lines,
                        unit_lines_rx.as_ref(),
                        cl_args.show_section_titles,
                        section == first_section,
                        cl_args.term_columns,
                    );
                }
            }
        }
    }

    Ok(())
}
