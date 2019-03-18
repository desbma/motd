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
    //
    // Memory
    //

    output_title("Memory usage");

    let mut mem_info = mem::MemInfo::new();

    // Get all memory usage info
    mem::get_mem_info(&mut mem_info);

    // Output memory usage
    mem::output_mem(mem_info, TERM_COLUMNS);

    //
    // Temps
    //

    output_title("Hardware temperatures");

    let mut temps = temp::TempDeque::new();

    // Hwmon temps
    temp::get_hwmon_temps(&mut temps);

    // Drive temps
    temp::get_drive_temps(&mut temps);

    // Output temps
    temp::output_temps(temps);
}
