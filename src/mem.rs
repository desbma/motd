use std::collections::HashMap;
use std::io::{BufReader,BufRead};
use std::fs::File;
use std::str::FromStr;

use colored::*;


const MEMBAR_LENGTH : u64 = 70;


/// Map of memory usage info
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

    // TODO autotruncate bar texts if needed
    // TODO center bar text

    let used_bar_text = format!("{:.1}GB ({:.1}%)",
                                used_mem_mb as f64 / 1024.0,
                                100.0 * used_mem_mb as f64 / total_mem_mb as f64).reversed();
    let used_bar = "█".repeat((MEMBAR_LENGTH * used_mem_mb / total_mem_mb) as usize - used_bar_text.len());

    let cached_bar_text = format!("{:.1}GB ({:.1}%)",
                                  (cache_mem_mb + buffer_mem_mb) as f64 / 1024.0,
                                  100.0 * (cache_mem_mb + buffer_mem_mb) as f64 / total_mem_mb as f64).dimmed().reversed();
    let cached_bar = "█".repeat((MEMBAR_LENGTH * (cache_mem_mb + buffer_mem_mb) / total_mem_mb) as usize - cached_bar_text.len()).dimmed();

    let free_bar_text = format!("{:.1}GB ({:.1}%)",
                                free_mem_mb as f64 / 1024.0,
                                100.0 * free_mem_mb as f64 / total_mem_mb as f64);
    let free_bar = " ".repeat((MEMBAR_LENGTH * free_mem_mb / total_mem_mb) as usize);


    println!("[{}{}{}{}{}{}] {:.1}GB ",
             used_bar_text,
             used_bar,
             cached_bar_text,
             cached_bar,
             free_bar_text,
             free_bar,
             total_mem_mb as f64 / 1024.0);
}
