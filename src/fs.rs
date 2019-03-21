use std::collections::VecDeque;
use std::ffi::CString;

use libc::{setmntent,getmntent};


/// Information on a filesystem
pub struct FsInfo {
    todo: String
}

/// Information on all filesystemd
pub type FsInfoDeque = VecDeque<FsInfo>;



/// Fetch filesystem information
pub fn get_fs_info() -> FsInfoDeque {
    let fs_info = FsInfoDeque::new();

    // Get mount list
    let path = CString::new("/proc/mounts").unwrap();
    let mode = CString::new("r").unwrap();
    let mount_file = unsafe { setmntent(path.as_ptr(), mode.as_ptr()) };
    if mount_file.is_null() {
        panic!("setmntent returned NULL");
    }

    // Loop over mounts
    loop {
        let mount = unsafe { getmntent(mount_file) };
        if mount.is_null() {
            break;
        }

        println!("{:?}", mount);
    }

    fs_info
}


/// Output filesystem information
pub fn output_fs_info(fs_info: FsInfoDeque) {

}
