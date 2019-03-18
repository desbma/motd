use std::collections::HashMap;
use std::io::{BufReader,BufRead};
use std::fs::File;
use std::str::FromStr;

use colored::*;


const MEMBAR_LENGTH : u64 = 70;


/// Map of memory usage info, unit is kB of page count
pub type MemInfo = HashMap<String, u64>;


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


pub fn output_mem(mem_info: MemInfo) {
    let total_mem_mb = mem_info["MemTotal"] / 1024;
    let cache_mem_mb = mem_info["Cached"] / 1024;
    let buffer_mem_mb = mem_info["Buffers"] / 1024;
    let free_mem_mb = mem_info["MemFree"] / 1024;
    let used_mem_mb  = total_mem_mb - cache_mem_mb - buffer_mem_mb - free_mem_mb;

    let keys = ["Dirty", "Cached", "Buffers"];
    let max_key_len = keys.iter().max_by_key(|x| x.len()).unwrap().len();
    for &key in keys.iter() {
        println!("{}: {}{: >5} MB ({: >4.1}%)",
                 key,
                 " ".repeat(max_key_len - key.len()),
                 mem_info[key] / 1024,
                 100.0 * mem_info[key] as f32 / mem_info["MemTotal"] as f32);
    }
    // TODO swap

    // TODO autotruncate bar texts if needed
    // TODO center bar text

    let mut used_bar_text = format!("{:.1}GB ({:.1}%)",
                                    used_mem_mb as f32 / 1024.0,
                                    100.0 * used_mem_mb as f32 / total_mem_mb as f32).reversed();
    let used_bar_len = (MEMBAR_LENGTH * used_mem_mb / total_mem_mb) as usize;
    if used_bar_text.len() > used_bar_len {
      used_bar_text = "".normal();
    }
    let used_bar = "█".repeat(used_bar_len - used_bar_text.len());

    let mut cached_bar_text = format!("{:.1}GB ({:.1}%)",
                                      (cache_mem_mb + buffer_mem_mb) as f32 / 1024.0,
                                      100.0 * (cache_mem_mb + buffer_mem_mb) as f32 / total_mem_mb as f32).dimmed().reversed();
    let cached_bar_len = (MEMBAR_LENGTH * (cache_mem_mb + buffer_mem_mb) / total_mem_mb) as usize;
    if cached_bar_text.len() > cached_bar_len {
      cached_bar_text = "".normal();
    }
    let cached_bar = "█".repeat(cached_bar_len - cached_bar_text.len()).dimmed();

    let mut free_bar_text = format!("{:.1}GB ({:.1}%)",
                                    free_mem_mb as f32 / 1024.0,
                                    100.0 * free_mem_mb as f32 / total_mem_mb as f32);
    let free_bar_len = (MEMBAR_LENGTH * free_mem_mb / total_mem_mb) as usize;
    if free_bar_text.len() > free_bar_len {
      free_bar_text = String::new()
    }
    let free_bar = " ".repeat(free_bar_len);


    println!("[{}{}{}{}{}{}] {:.1}GB ",
             used_bar_text,
             used_bar,
             cached_bar_text,
             cached_bar,
             free_bar_text,
             free_bar,
             total_mem_mb as f32 / 1024.0);
}
