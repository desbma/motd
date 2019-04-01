use std::cmp;
use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::io;
use std::mem;

use ansi_term::Colour::*;
use ansi_term::Style;
use bytesize::ByteSize;
use libc::{endmntent, getmntent, setmntent, statvfs};

/// Information on a filesystem
pub struct FsInfo {
    mount_path: String,
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
            mount_path = CStr::from_ptr((*mount).mnt_dir)
                .to_str()
                .unwrap()
                .to_string();
            fs_type = CStr::from_ptr((*mount).mnt_type)
                .to_str()
                .unwrap()
                .to_string();
        }

        // Exclude some cases
        if (fs_type == "devtmpfs")
            || (fs_type == "autofs")
            || fs_type.starts_with("fuse.")
            || mount_path.starts_with("/dev/")
            || (mount_path == "/run")
            || mount_path.starts_with("/run/")
            || mount_path.starts_with("/sys/")
        {
            continue;
        }

        // Get filesystem info
        let mut new_fs_info = FsInfo {
            mount_path,
            used_bytes: 0,
            total_bytes: 0,
        };
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
    unsafe { endmntent(mount_file) }; // endmntent always returns 1

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

/// Generate a bar to represent filesystem usage
pub fn get_fs_bar(fs_info: &FsInfo, length: usize, style: Style) -> String {
    let bar_text = format!(
        "{} / {} ({:.1}%)",
        ByteSize(fs_info.used_bytes),
        ByteSize(fs_info.total_bytes),
        100.0 * fs_info.used_bytes as f32 / fs_info.total_bytes as f32
    );

    // Center bar text inside fill chars
    let bar_text_len = bar_text.len();
    let fill_count_before = (length - 2 - bar_text_len) / 2;
    let chars_used = ((length - 2) as u64 * fs_info.used_bytes / fs_info.total_bytes) as usize;

    let bar_char = '█';

    let pos1 = cmp::min(chars_used, fill_count_before);
    let pos2 = fill_count_before;
    let pos3 = cmp::max(
        fill_count_before,
        cmp::min(chars_used, fill_count_before + bar_text_len),
    );
    let pos4 = fill_count_before + bar_text_len;
    let pos5 = cmp::max(chars_used, fill_count_before + bar_text_len);

    format!(
        "▕{}{}{}{}{}{}▏",
        style.paint(bar_char.to_string().repeat(pos1)),
        style.paint(' '.to_string().repeat(pos2 - pos1)),
        style.reverse().paint(&bar_text[0..(pos3 - pos2)]),
        style.paint(&bar_text[(pos3 - pos2)..]),
        style.paint(bar_char.to_string().repeat(pos5 - pos4)),
        style.paint(' '.to_string().repeat(length - 2 - pos5))
    )
}

/// Output filesystem information
pub fn output_fs_info(fs_info: FsInfoVec, term_width: usize) -> VecDeque<String> {
    let mut lines: VecDeque<String> = VecDeque::new();

    let max_path_len = fs_info
        .iter()
        .max_by_key(|x| x.mount_path.len())
        .unwrap()
        .mount_path
        .len();

    for cur_fs_info in fs_info {
        let text_style;
        let fs_usage = cur_fs_info.used_bytes as f32 / cur_fs_info.total_bytes as f32;
        if fs_usage >= 0.95 {
            text_style = Red.normal();
        } else if fs_usage >= 0.85 {
            text_style = Yellow.normal();
        } else {
            text_style = Style::new();
        }

        lines.push_back(format!(
            "{}{} {}",
            text_style.paint(&cur_fs_info.mount_path),
            text_style.paint(" ".repeat(max_path_len - cur_fs_info.mount_path.len())),
            get_fs_bar(
                &cur_fs_info,
                cmp::max(term_width - max_path_len - 1, 30),
                text_style
            )
        ));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_fs_info() {
        assert_eq!(output_fs_info(vec![FsInfo{mount_path: "/foo/bar".to_string(),
                                              used_bytes: 234560,
                                              total_bytes: 7891011},
                                       FsInfo{mount_path: "/foo/baz".to_string(),
                                              used_bytes: 2345600000,
                                              total_bytes: 7891011000}],
                                  60),
                   ["/foo/bar ▕█           \u{1b}[7m\u{1b}[0m234.6 KB / 7.9 MB (3.0%)             ▏",
                    "/foo/baz ▕█████████████\u{1b}[7m2\u{1b}[0m.3 GB / 7.9 GB (29.7%)             ▏"]);
    }

    #[test]
    fn test_get_fs_bar() {
        assert_eq!(get_fs_bar(&FsInfo{mount_path: "/foo/bar".to_string(),
                                      used_bytes: 23456,
                                      total_bytes: 7891011},
                              40,
                              Red.normal()),
                   "▕\u{1b}[31m\u{1b}[0m\u{1b}[31m       \u{1b}[0m\u{1b}[7;31m\u{1b}[0m\u{1b}[31m23.5 KB / 7.9 MB (0.3%)\u{1b}[0m\u{1b}[31m\u{1b}[0m\u{1b}[31m        \u{1b}[0m▏");
        assert_eq!(
            get_fs_bar(
                &FsInfo {
                    mount_path: "/foo/bar".to_string(),
                    used_bytes: 0,
                    total_bytes: 7891011
                },
                40,
                Style::new()
            ),
            "▕         \u{1b}[7m\u{1b}[0m0 B / 7.9 MB (0.0%)          ▏"
        );
        assert_eq!(
            get_fs_bar(
                &FsInfo {
                    mount_path: "/foo/bar".to_string(),
                    used_bytes: 434560,
                    total_bytes: 7891011
                },
                40,
                Style::new()
            ),
            "▕██     \u{1b}[7m\u{1b}[0m434.6 KB / 7.9 MB (5.5%)       ▏"
        );
        assert_eq!(
            get_fs_bar(
                &FsInfo {
                    mount_path: "/foo/bar".to_string(),
                    used_bytes: 4891011000,
                    total_bytes: 7891011000
                },
                40,
                Style::new()
            ),
            "▕███████\u{1b}[7m4.9 GB / 7.9 GB \u{1b}[0m(62.0%)        ▏"
        );
        assert_eq!(
            get_fs_bar(
                &FsInfo {
                    mount_path: "/foo/bar".to_string(),
                    used_bytes: 4891011000,
                    total_bytes: 7891011000
                },
                30,
                Style::new()
            ),
            "▕██\u{1b}[7m4.9 GB / 7.9 GB\u{1b}[0m (62.0%)   ▏"
        );
        assert_eq!(get_fs_bar(&FsInfo{mount_path: "/foo/bar".to_string(),
                                      used_bytes: 4891011000,
                                      total_bytes: 7891011000},
                              50,
                              Style::new()),
                   "▕████████████\u{1b}[7m4.9 GB / 7.9 GB (\u{1b}[0m62.0%)             ▏");
        assert_eq!(
            get_fs_bar(
                &FsInfo {
                    mount_path: "/foo/bar".to_string(),
                    used_bytes: 6891011000000,
                    total_bytes: 7891011000000
                },
                40,
                Style::new()
            ),
            "▕███████\u{1b}[7m6.9 TB / 7.9 TB (87.3%)\u{1b}[0m███     ▏"
        );
        assert_eq!(get_fs_bar(&FsInfo{mount_path: "/foo/bar".to_string(),
                                      used_bytes: 7891011000000,
                                      total_bytes: 7891011000000},
                              40,
                              Style::new()),
                   "▕███████\u{1b}[7m7.9 TB / 7.9 TB (100.0%)\u{1b}[0m███████▏");
    }
}
