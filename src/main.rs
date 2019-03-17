use std::collections::HashMap;

mod temp;


fn output_title(title: &str) {
    println!("──────── {}", title);
}


fn main() {
    //
    // Memory
    //

    output_title("Memory usage");

    let mut mem_info: HashMap<String, u32> = HashMap::new();

    // Geta all memory usage info
    //get_mem_info(&mut mem_info);

    //
    // Temps
    //

    output_title("Hardware temperatures");

    let mut temps: temp::TempDeque = temp::TempDeque::new();

    // Hwmon temps
    temp::get_hwmon_temps(&mut temps);

    // Drive temps
    temp::get_drive_temps(&mut temps);

    // Output temps
    temp::output_temps(temps);
}
