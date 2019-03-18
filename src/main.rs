use std::thread;

mod mem;
mod temp;


/// Terminal column count (width)
const TERM_COLUMNS : usize = 80;  // TODO Get this dynamically?


fn output_title(title: &str) {
    println!("\n{:─^width$}",
             format!(" {} ", title),
             width=TERM_COLUMNS);
}


fn main() {
    if cfg!(feature = "worker_thread") {
        let mut temps = temp::TempDeque::new();

        // Fetch temps in a background thread

        let temp_fetcher_thread = thread::Builder::new().name("temp_fetcher".to_string()).spawn(move || {
            // Get temps
            temp::get_hwmon_temps(&mut temps);
            temp::get_drive_temps(&mut temps);

            temps
        }).unwrap();


        output_title("Memory usage");

        let mut mem_info = mem::MemInfo::new();

        // Get all memory usage info
        mem::get_mem_info(&mut mem_info);

        // Output memory usage
        mem::output_mem(mem_info, TERM_COLUMNS);


        output_title("Hardware temperatures");

        // Output temps
        temps = temp_fetcher_thread.join().unwrap();
        temp::output_temps(temps);
    }
    else {
        output_title("Memory usage");

        let mut mem_info = mem::MemInfo::new();

        // Get all memory usage info
        mem::get_mem_info(&mut mem_info);

        // Output memory usage
        mem::output_mem(mem_info, TERM_COLUMNS);


        output_title("Hardware temperatures");

        // Get temps
        let mut temps = temp::TempDeque::new();
        temp::get_hwmon_temps(&mut temps);
        temp::get_drive_temps(&mut temps);

        // Output temps
        temp::output_temps(temps);
    }
}
