use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::{BufReader,BufRead};
use std::fs::File;
use std::str::FromStr;

use ansi_term::Style;


/// Length of memory bar in chars
const MEMBAR_LENGTH : u64 = 70;


/// Map of memory usage info, unit is kB of page count
pub type MemInfo = HashMap<String, u64>;


/// Fetch memory usage info from procfs
pub fn get_mem_info(mem_info: &mut MemInfo) {
    let file = File::open("/proc/meminfo").unwrap();
    let reader = BufReader::new(file);
    for line in reader.lines() {
        // Parse line
        let line_str = line.unwrap();
        let mut tokens_it = line_str.split(':');
        let key = tokens_it.next().unwrap().to_string();
        let val_str = tokens_it.next().unwrap().trim_start();
        let val = u64::from_str(val_str.split(' ').next().unwrap()).unwrap();

        // Store info
        mem_info.insert(key, val);
    }
}


/// Memory bar section
struct BarPart {
    /// Section text
    label: String,
    /// Percentage of full bar this section should fill
    prct: f32,
    /// Bar text style
    text_style: Style,
    /// Bar fill char style
    fill_style: Style,
    /// Char to use to fill bar
    bar_char: char,
}


/// Print memory bar
fn output_bar(parts: VecDeque<BarPart>, length: u64) {
    let mut full_bar: String = "[".to_string();
    for part in parts {
        let part_len = (length as f32 * part.prct / 100.0) as usize;
        if part.label.len() <= part_len {
            // TODO center bar text
            full_bar += &part.text_style.paint(&part.label).to_string();
            full_bar += &part.fill_style.paint(&part.bar_char.to_string().repeat(part_len - part.label.len())).to_string();
        }
        else {
            full_bar += &part.fill_style.paint(&part.bar_char.to_string().repeat(part_len)).to_string();
        }
    }

    println!("{}", full_bar);
}


/// Output all memory info
pub fn output_mem(mem_info: MemInfo) {
    let total_mem_mb = mem_info["MemTotal"] / 1024;
    let cache_mem_mb = mem_info["Cached"] / 1024;
    let buffer_mem_mb = mem_info["Buffers"] / 1024;
    let free_mem_mb = mem_info["MemFree"] / 1024;
    let used_mem_mb  = total_mem_mb - cache_mem_mb - buffer_mem_mb - free_mem_mb;

    let keys = ["MemTotal", "MemFree", "Dirty", "Cached", "Buffers"];
    let max_key_len = keys.iter().max_by_key(|x| x.len()).unwrap().len();
    for &key in keys.iter() {
        print!("{}: {}{: >5} MB",
               key,
               " ".repeat(max_key_len - key.len()),
               mem_info[key] / 1024);
        if key != "MemTotal" {
            println!(" ({: >4.1}%)", 100.0 * mem_info[key] as f32 / mem_info["MemTotal"] as f32);
        }
        else {
            println!("");
        }
    }
    // TODO swap

    let mut bar_parts = VecDeque::new();

    let used_prct = 100.0 * used_mem_mb as f32 / total_mem_mb as f32;
    let used_bar_text = format!("{:.1}GB ({:.1}%)",
                                used_mem_mb as f32 / 1024.0,
                                used_prct);
    bar_parts.push_back(BarPart{label: used_bar_text,
                                prct: used_prct,
                                text_style: Style::new().reverse(),
                                fill_style: Style::new(),
                                bar_char: '█'});

    let cached_prct = 100.0 * (cache_mem_mb + buffer_mem_mb) as f32 / total_mem_mb as f32;
    let cached_bar_text = format!("{:.1}GB ({:.1}%)",
                                  (cache_mem_mb + buffer_mem_mb) as f32 / 1024.0,
                                  cached_prct);
    bar_parts.push_back(BarPart{label: cached_bar_text,
                                prct: cached_prct,
                                text_style: Style::new().dimmed().reverse(),
                                fill_style: Style::new().dimmed(),
                                bar_char: '█'});

    let free_prct = 100.0 * free_mem_mb as f32 / total_mem_mb as f32;
    let free_bar_text = format!("{:.1}GB ({:.1}%)",
                                free_mem_mb as f32 / 1024.0,
                                free_prct);
    bar_parts.push_back(BarPart{label: free_bar_text,
                                prct: free_prct,
                                text_style: Style::new(),
                                fill_style: Style::new(),
                                bar_char: ' '});

    output_bar(bar_parts, MEMBAR_LENGTH);
}
