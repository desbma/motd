use std::cmp;
use std::iter::Iterator;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::thread;

use ansi_term::Colour::*;
use clap::{App, Arg};
use itertools::Itertools;

use crate::module::ModuleData;

mod fmt;
mod fs;
mod load;
mod mem;
mod module;
mod net;
mod systemd;
mod temp;

/// Output section
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
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

/// Message shown when there is a delay
const LOADING_MSG: &str = "Loading…";

/// Output section header to stdout
fn output_title(title: &str, columns: usize, new_line: bool) {
    if new_line {
        println!();
    }
    println!("{:─^width$}", format!(" {} ", title), width = columns);
}

/// Output section title and lines
fn output_section(
    title: &str,
    lines: Result<String, String>,
    show_title: bool,
    first_section: bool,
    delayed: bool,
    columns: usize,
) {
    if delayed {
        eprint!("\r{}\r", " ".repeat(LOADING_MSG.len()));
    }
    match lines {
        Ok(lines) => {
            if !lines.is_empty() {
                if show_title {
                    output_title(title, columns, !first_section);
                } else if !first_section {
                    println!();
                }

                print!("{}", lines);
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
fn section_to_letter(section: &Section) -> &'static str {
    match section {
        Section::Load => "l",
        Section::Mem => "m",
        Section::Swap => "s",
        Section::FS => "f",
        Section::Temps => "t",
        Section::Network => "n",
        Section::SDFailedUnits => "u",
    }
}

/// Get letter from Section
fn pretty_section_name(section: &Section) -> &str {
    match section {
        Section::Load => "Load",
        Section::Mem => "Memory usage",
        Section::Swap => "Swap usage",
        Section::FS => "Filesystem usage",
        Section::Temps => "Hardware temperatures",
        Section::Network => "Network",
        Section::SDFailedUnits => "Systemd failed units",
    }
}

/// Get Section from letter
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
    let sections_str: Vec<&'static str> = vec![
        Section::Load,
        Section::Mem,
        Section::Swap,
        Section::FS,
        Section::Temps,
        Section::Network,
        Section::SDFailedUnits,
    ]
    .iter()
    .map(section_to_letter)
    .collect();
    let default_sections_string = sections_str.join(",");

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

fn main() -> anyhow::Result<()> {
    let cl_args = parse_cl_args();

    module::CPU_COUNT.store(num_cpus::get(), Ordering::SeqCst);
    module::TERM_COLUMNS.store(cl_args.term_columns, Ordering::SeqCst);

    let mut section_futs: Vec<thread::JoinHandle<anyhow::Result<ModuleData>>> = Vec::new();
    section_futs.reserve(cl_args.sections.len());
    for section in &cl_args.sections {
        let section_fut = match section {
            Section::Load => thread::spawn(load::fetch),
            Section::Mem => thread::spawn(mem::fetch),
            Section::Swap => thread::spawn(|| {
                // TODO fetch only once?
                let mi = mem::fetch()?;
                if let ModuleData::Memory(mi) = mi {
                    Ok(ModuleData::Swap(mem::SwapInfo::from(mi)))
                } else {
                    unreachable!();
                }
            }),
            Section::FS => thread::spawn(fs::fetch),
            Section::Temps => thread::spawn(temp::fetch),
            Section::SDFailedUnits => thread::spawn(systemd::fetch),
            Section::Network => thread::spawn(net::fetch),
        };
        section_futs.push(section_fut);
    }

    for (i, (section_fut, section)) in section_futs
        .into_iter()
        .zip(cl_args.sections.iter())
        .enumerate()
    {
        let delayed = !section_fut.is_finished();
        if delayed {
            eprint!("{}", LOADING_MSG);
        }
        let lines = section_fut
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {:?}", e))?
            .map(|i| format!("{}", i))
            .map_err(|e| format!("{}", e));
        output_section(
            pretty_section_name(section),
            lines,
            cl_args.show_section_titles,
            i == 0,
            delayed,
            cl_args.term_columns,
        );
    }

    Ok(())
}
