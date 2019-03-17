use std::collections::HashMap;
use std::io::{BufReader,BufRead};
use std::fs::File;
use std::str::FromStr;


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
    for (key, val) in mem_info {
        println!("{}={}", key, val);
    }
}
