use std::ffi::{CStr,CString};
use std::io;
use std::mem;

use bytesize::ByteSize;
use libc::{endmntent,getmntent,setmntent,statvfs};


/// Information on a filesystem
pub struct FsInfo {
    mount_path: String,
    fs_type: String,
    used_bytes: u64,
    total_bytes: u64,
}

/// Information on all filesystems
pub type FsInfoVec = Vec<FsInfo>;



/// Fetch filesystem information for all filesystems
pub fn get_fs_info() -> FsInfoVec {
    let mut fs_info = FsInfoVec::new();

    // Open mount list file
    // Note: /etc/mtab is a symlink to /proc/self/mounts
    let path = CString::new("/proc/mounts").unwrap();
    let mode = CString::new("r").unwrap();
    let mount_file = unsafe { setmntent(path.as_ptr(), mode.as_ptr()) };
    if mount_file.is_null() {
        panic!();
    }

    // Loop over mounts
    loop {
        let mount = unsafe { getmntent(mount_file) };
        if mount.is_null() {
            break;
        }
        let mount_path;
        let fs_type;
        unsafe {
            mount_path = CStr::from_ptr((*mount).mnt_dir).to_str().unwrap().to_string();
            fs_type = CStr::from_ptr((*mount).mnt_type).to_str().unwrap().to_string();
        }

        // Exclude some cases
        if (fs_type == "devtmpfs") ||
           fs_type.starts_with("fuse.") ||
           mount_path.starts_with("/dev/") ||
           (mount_path == "/run") ||
           mount_path.starts_with("/run/") ||
           mount_path.starts_with("/sys/") {
            continue;
        }

        // Get filesystem info
        let mut new_fs_info = FsInfo{mount_path: mount_path,
                                     fs_type: fs_type,
                                     used_bytes: 0,
                                     total_bytes: 0};
        new_fs_info = match fill_fs_info(new_fs_info) {
            Ok(fsi) => fsi,
            Err(_e) => continue,
        };
        fs_info.push(new_fs_info);
    }

    // Close mount list file
    unsafe { endmntent(mount_file) };  // endmntent always returns 1

    fs_info.sort_by(|a, b| a.mount_path.cmp(&b.mount_path));

    fs_info
}


/// Fetch detailed filesystem information
fn fill_fs_info(fs_info: FsInfo) -> Result<FsInfo, io::Error> {
    let mut fs_stat: statvfs = unsafe { mem::zeroed() };
    let mount_point = CString::new(fs_info.mount_path.to_owned()).unwrap();
    let rc = unsafe { statvfs(mount_point.as_ptr(), &mut fs_stat) };
    if rc != 0 {
        //println!("{} {:?}", fs_info.mount_path, io::Error::last_os_error());
        return Err(io::Error::last_os_error());
    }

    let mut fs_info = fs_info;
    fs_info.total_bytes = fs_stat.f_blocks * fs_stat.f_bsize;
    fs_info.used_bytes = fs_info.total_bytes - fs_stat.f_bfree * fs_stat.f_bsize;

    Ok(fs_info)
}


/// Output filesystem information
pub fn output_fs_info(fs_info: FsInfoVec) {
    for cur_fs_info in fs_info {
        if cur_fs_info.total_bytes == 0 {
            // procfs, sysfs...
            continue;
        }

        println!("{} [{}] {} / {} ({:.1}%)",
                 cur_fs_info.mount_path,
                 cur_fs_info.fs_type,
                 ByteSize(cur_fs_info.used_bytes),
                 ByteSize(cur_fs_info.total_bytes),
                 100.0 * cur_fs_info.used_bytes as f32 / cur_fs_info.total_bytes as f32);
    }
}
