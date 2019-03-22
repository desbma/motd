use std::cmp;
use std::ffi::{CStr,CString};
use std::io;
use std::mem;

use ansi_term::Style;
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
        if new_fs_info.total_bytes == 0 {
            // procfs, sysfs...
            continue;
        }
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


pub fn output_fs_bar(fs_info: &FsInfo, length: usize, style: Style) -> String {
    let bar_text = format!("{} / {} ({:.1}%)",
                           ByteSize(fs_info.used_bytes),
                           ByteSize(fs_info.total_bytes),
                           100.0 * fs_info.used_bytes as f32 / fs_info.total_bytes as f32);

    // Center bar text inside fill chars
    let bar_text_len = bar_text.len();
    let fill_count_before = (length - 2 - bar_text_len) / 2;
    let mut fill_count_after = fill_count_before;
    if (length - 2 - bar_text_len) % 2 == 1 {
        fill_count_after += 1;
    }
    let chars_used = ((length - 2) as u64 * fs_info.used_bytes / fs_info.total_bytes) as usize;

    let bar_char = '█';

    let pos1 = cmp::min(chars_used, fill_count_before);
    let pos2 = fill_count_before;
    let pos3 = cmp::max(fill_count_before, cmp::min(chars_used, fill_count_before + bar_text_len));
    let pos4 = fill_count_before + bar_text_len;
    let pos5 = cmp::max(chars_used, fill_count_before + bar_text_len);

    format!("[{}{}{}{}{}{}]",
            style.paint(bar_char.to_string().repeat(pos1)),
            style.paint(' '.to_string().repeat(pos2 - pos1)),
            style.reverse().paint(&bar_text[0..(pos3 - pos2)]),
            style.paint(&bar_text[cmp::max(0, pos3 - pos2)..]),
            style.paint(bar_char.to_string().repeat(pos5 - pos4)),
            style.paint(' '.to_string().repeat(length - 2 - pos5)))
}


/// Output filesystem information
pub fn output_fs_info(fs_info: FsInfoVec, term_width: usize) {
    let max_path_len = fs_info.iter().max_by_key(|x| x.mount_path.len()).unwrap().mount_path.len();

    for cur_fs_info in fs_info {
        println!("{}{} {}",
                 cur_fs_info.mount_path,
                 " ".repeat(max_path_len - cur_fs_info.mount_path.len()),
                 output_fs_bar(&cur_fs_info, cmp::max(term_width - max_path_len - 1, 30), Style::new()));
    }
}