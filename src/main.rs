use std::fs;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use std::process;


fn normalize_drive_path(path: &str) -> String {
    let mut path_string = path.to_string();
    let fs_path = Path::new(path);

    if fs::symlink_metadata(path).unwrap().file_type().is_symlink() {
        let mut real_path = fs::read_link(path).unwrap();
        if !real_path.is_absolute() {
            let dirname = fs_path.parent().unwrap();
            real_path = dirname.join(real_path).canonicalize().unwrap();
        }
        path_string = real_path.into_os_string().into_string().unwrap();
    }

    path_string
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

    // Parse & output
    let drives_data: Vec<&str> = data.split("|").collect();
    for drive_data in drives_data.chunks_exact(5) {
        let drive_path = normalize_drive_path(drive_data[1]);
        let pretty_name = drive_data[2];
        let temp = drive_data[3];
        let temp_unit = drive_data[4].replace("C", "°C");
        println!("{} ({});{} {}", drive_path, pretty_name, temp, temp_unit);
    }
}
