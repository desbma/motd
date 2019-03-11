use std::fs;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use std::process;


fn normalize_drive_path(drive_path : &str) {

}


fn main() {
    // Connect
    let stream = TcpStream::connect("127.0.0.1:7634");  // TODO port const
    let mut stream = match stream {
        Ok(s) => s,
        Err(_e) => process::exit(0),  // TODO use EXIT_SUCCESS
    };

    // Read
    let mut data = String::new();
    stream.read_to_string(&mut data).unwrap();

    // Parse
    let drives_data: Vec<&str> = data.split("|").collect();
    println!("{}", drives_data.len());
    for drive_data in drives_data.chunks_exact(5) {
        let drive_path_str = drive_data[1];
        let mut drive_path_str2 = drive_data[1].to_string();
        let drive_path = Path::new(drive_path_str);

        // Resolve to canonical drive path if needed
        if fs::symlink_metadata(drive_path_str).unwrap().file_type().is_symlink() {
            let mut real_path = fs::read_link(drive_path_str).unwrap();
            if !real_path.is_absolute() {
                let dirname = drive_path.parent().unwrap();
                real_path = dirname.join(real_path).canonicalize().unwrap();;
            }
            drive_path_str2 = real_path.into_os_string().into_string().unwrap();
        }
        println!("{}", drive_path_str2);
    }

    // name = "%s (%s)" % (drive_path, drive_data[2])
    // temp = " ".join(drive_data[3:5]).replace("C", "°C")
    // print("%s:;%s" % (name, temp))


}
