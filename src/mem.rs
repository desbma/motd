use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::{BufReader,BufRead};
use std::fs::File;
use std::str::FromStr;

use ansi_term::Style;


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
    label: Vec<String>,
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
fn output_bar(parts: VecDeque<BarPart>, length: usize) -> String {
    let mut full_bar = "[".to_string();
    for part in parts {
        let part_len = ((length - 2) as f32 * part.prct / 100.0).round() as usize;
        // Build longest label that fits
        let mut label = String::new();
        for label_part in part.label {
            if label.len() + label_part.len() <= part_len {
                label += &label_part;
            }
            else {
                break;
            }
        }

        // Center bar text inside fill chars
        let label_len = label.len();
        let fill_count_before = (part_len - label_len) / 2;
        let mut fill_count_after = fill_count_before;
        if (part_len - label_len) % 2 == 1 {
            fill_count_after += 1;
        }
        full_bar += &part.fill_style.paint(&part.bar_char.to_string().repeat(fill_count_before)).to_string();
        full_bar += &part.text_style.paint(&label).to_string();
        full_bar += &part.fill_style.paint(&part.bar_char.to_string().repeat(fill_count_after)).to_string();
    }

    format!("{}]", full_bar)
}


/// Print memory stat numbers
fn output_mem_stats(mem_info: &MemInfo, keys: Vec<&str>, total_key: &str) -> VecDeque<String> {
    let mut lines: VecDeque<String> = VecDeque::new();

    let max_key_len = keys.iter().max_by_key(|x| x.len()).unwrap().len();
    for &key in keys.iter() {
        let mut line: String = format!("{}: {}{: >5} MB",
                                       key,
                                       " ".repeat(max_key_len - key.len()),
                                       mem_info[key] / 1024);
        if key != total_key {
            line += &format!(" ({: >4.1}%)", 100.0 * mem_info[key] as f32 / mem_info[total_key] as f32);
        }
        lines.push_back(line);
    }

    lines
}


/// Output all memory info
pub fn output_mem(mem_info: MemInfo, term_width: usize) -> VecDeque<String> {
    let mut lines: VecDeque<String> = VecDeque::new();

    //
    // Memory
    //

    lines.extend(output_mem_stats(&mem_info, vec!["MemTotal", "MemFree", "Dirty", "Cached", "Buffers"], "MemTotal"));

    let total_mem_mb = mem_info["MemTotal"] / 1024;
    let cache_mem_mb = mem_info["Cached"] / 1024;
    let buffer_mem_mb = mem_info["Buffers"] / 1024;
    let free_mem_mb = mem_info["MemFree"] / 1024;
    let used_mem_mb  = total_mem_mb - cache_mem_mb - buffer_mem_mb - free_mem_mb;

    let mut mem_bar_parts = VecDeque::new();

    let used_prct = 100.0 * used_mem_mb as f32 / total_mem_mb as f32;
    let used_bar_text: Vec<String> = vec!["Used".to_string(),
                                          format!(" {:.1}GB", used_mem_mb as f32 / 1024.0),
                                          format!(" ({:.1}%)", used_prct)];
    mem_bar_parts.push_back(BarPart{label: used_bar_text,
                                    prct: used_prct,
                                    text_style: Style::new().reverse(),
                                    fill_style: Style::new(),
                                    bar_char: '█'});

    let cached_prct = 100.0 * (cache_mem_mb + buffer_mem_mb) as f32 / total_mem_mb as f32;
    let cached_bar_text: Vec<String> = vec!["Cached".to_string(),
                                            format!(" {:.1}GB", (cache_mem_mb + buffer_mem_mb) as f32 / 1024.0),
                                            format!(" ({:.1}%)", cached_prct)];
    mem_bar_parts.push_back(BarPart{label: cached_bar_text,
                                    prct: cached_prct,
                                    text_style: Style::new().dimmed().reverse(),
                                    fill_style: Style::new().dimmed(),
                                    bar_char: '█'});

    let free_prct = 100.0 * free_mem_mb as f32 / total_mem_mb as f32;
    let free_bar_text: Vec<String> = vec!["Free".to_string(),
                                          format!(" {:.1}GB", free_mem_mb as f32 / 1024.0),
                                          format!(" ({:.1}%)", free_prct)];
    mem_bar_parts.push_back(BarPart{label: free_bar_text,
                                    prct: free_prct,
                                    text_style: Style::new(),
                                    fill_style: Style::new(),
                                    bar_char: ' '});

    lines.push_back(output_bar(mem_bar_parts, term_width));

    //
    // Swap
    //

    if mem_info["SwapTotal"] > 0 {
        lines.extend(output_mem_stats(&mem_info, vec!["SwapTotal", "SwapFree"], "SwapTotal"));

        let total_swap_mb = mem_info["SwapTotal"] / 1024;
        let free_swap_mb = mem_info["SwapFree"] / 1024;
        let used_swap_mb  = total_swap_mb - free_swap_mb;

        let mut swap_bar_parts = VecDeque::new();

        let used_prct = 100.0 * used_swap_mb as f32 / total_swap_mb as f32;
        let used_bar_text: Vec<String> = vec!["Used".to_string(),
                                              format!(" {:.1}GB", used_swap_mb as f32 / 1024.0),
                                              format!(" ({:.1}%)", used_prct)];
        swap_bar_parts.push_back(BarPart{label: used_bar_text,
                                         prct: used_prct,
                                         text_style: Style::new().reverse(),
                                         fill_style: Style::new(),
                                         bar_char: '█'});

        let free_prct = 100.0 * free_swap_mb as f32 / total_swap_mb as f32;
        let free_bar_text: Vec<String> = vec!["Swap free".to_string(),
                                              format!(" {:.1}GB", free_swap_mb as f32 / 1024.0),
                                              format!(" ({:.1}%)", free_prct)];
        swap_bar_parts.push_back(BarPart{label: free_bar_text,
                                         prct: free_prct,
                                         text_style: Style::new(),
                                         fill_style: Style::new(),
                                         bar_char: ' '});

        lines.push_back(output_bar(swap_bar_parts, term_width));
    }

    lines
}
