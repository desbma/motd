use std::{
    cmp,
    collections::HashSet,
    ffi::{CStr, CString, OsStr},
    fmt, io, mem,
    os::unix::ffi::OsStrExt as _,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

use ansi_term::{
    Colour::{Red, Yellow},
    Style,
};
use libc::{endmntent, getmntent, setmntent, statvfs};

use crate::{
    config,
    fmt::format_kmgt,
    module::{ModuleData, TERM_COLUMNS},
};

const MIN_FS_BAR_LEN: usize = 30;

/// Information on a filesystem
pub(crate) struct FsMountInfo {
    mount_path: PathBuf,
    used_bytes: u64,
    total_bytes: u64,
}

/// Information on all filesystems
pub(crate) struct FsInfo {
    mounts: Vec<FsMountInfo>,
}

/// Fetch filesystem information for all filesystems
pub(crate) fn fetch(cfg: &config::FsConfig) -> anyhow::Result<ModuleData> {
    let mut mounts = Vec::new();

    // Open mount list file
    // Note: /etc/mtab is a symlink to /proc/self/mounts
    let path = CString::new("/proc/mounts")?;
    let mode = CString::new("r")?;
    // SAFETY: libc call
    let mount_file = unsafe { setmntent(path.as_ptr(), mode.as_ptr()) };
    anyhow::ensure!(!mount_file.is_null(), "setmntent failed");

    // Loop over mounts
    let mut known_devices = HashSet::new();
    loop {
        // SAFETY: libc call
        let mount = unsafe { getmntent(mount_file) };
        if mount.is_null() {
            break;
        }
        let mount_path_raw;
        let fs_type;
        let fs_dev;
        // SAFETY: get getmntend output
        unsafe {
            mount_path_raw = CStr::from_ptr((*mount).mnt_dir);
            fs_type = CStr::from_ptr((*mount).mnt_type).to_str()?;
            fs_dev = CStr::from_ptr((*mount).mnt_fsname).to_str()?;
        }
        let mount_path: &Path = OsStr::from_bytes(mount_path_raw.to_bytes()).as_ref();

        // Exclusions
        if cfg.mount_type_blacklist.iter().any(|r| r.is_match(fs_type)) {
            continue;
        }
        if let Some(mount_path) = mount_path.to_str() {
            if cfg
                .mount_path_blacklist
                .iter()
                .any(|r| r.is_match(mount_path))
            {
                continue;
            }
        }

        // Exclude mounts of devices already mounted (avoids duplicate for bind mounts or btrfs subvolumes)
        if fs_dev.starts_with('/') {
            if known_devices.contains(fs_dev) {
                continue;
            }
            known_devices.insert(fs_dev.to_owned());
        }

        // Get filesystem info
        let Ok(mount_info) = fetch_mount_info(mount_path) else {
            continue;
        };
        if mount_info.total_bytes == 0 {
            // procfs, sysfs...
            continue;
        }
        mounts.push(mount_info);
    }

    // Close mount list file
    // SAFETY: libc call
    unsafe {
        endmntent(mount_file);
    } // endmntent always returns 1

    mounts.sort_by(|a, b| a.mount_path.cmp(&b.mount_path));

    Ok(ModuleData::Fs(FsInfo { mounts }))
}

/// Fetch detailed filesystem information
#[allow(clippy::allow_attributes, clippy::unnecessary_cast)] // 32/64 bits
fn fetch_mount_info(mount_path: &Path) -> Result<FsMountInfo, io::Error> {
    // SAFETY: libc call arg
    let mut fs_stat: statvfs = unsafe { mem::zeroed() };
    let mount_point = CString::new(mount_path.as_os_str().as_bytes())?;
    // SAFETY: libc call
    let rc = unsafe { statvfs(mount_point.as_ptr(), &raw mut fs_stat) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }

    let total_bytes = fs_stat.f_blocks * fs_stat.f_bsize as u64;
    let used_bytes = total_bytes - fs_stat.f_bfree * fs_stat.f_bsize as u64;

    Ok(FsMountInfo {
        total_bytes,
        used_bytes,
        mount_path: mount_path.to_path_buf(),
    })
}

/// Generate a bar to represent filesystem usage
#[expect(clippy::string_slice)]
pub(crate) fn get_fs_bar(mount_info: &FsMountInfo, length: usize, style: Style) -> String {
    assert!(length >= MIN_FS_BAR_LEN);

    let bar_text = format!(
        "{} / {} ({:.1}%)",
        format_kmgt(mount_info.used_bytes, "B"),
        format_kmgt(mount_info.total_bytes, "B"),
        100.0 * mount_info.used_bytes as f32 / mount_info.total_bytes as f32
    );

    // Center bar text inside fill chars
    let bar_text_len = bar_text.len();
    let fill_count_before = (length - 2 - bar_text_len) / 2;
    let chars_used =
        ((length - 2) as u64 * mount_info.used_bytes / mount_info.total_bytes) as usize;

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
        "{}{}{}{}{}{}{}{}",
        style.paint("▕"),
        style.paint(bar_char.to_string().repeat(pos1)),
        style.paint(' '.to_string().repeat(pos2 - pos1)),
        style.reverse().paint(&bar_text[0..(pos3 - pos2)]),
        style.paint(&bar_text[(pos3 - pos2)..]),
        style.paint(bar_char.to_string().repeat(pos5 - pos4)),
        style.paint(' '.to_string().repeat(length - 2 - pos5)),
        style.paint("▏"),
    )
}

fn ellipsis(s: &str, max_len: usize) -> String {
    assert!(max_len >= 1);

    if s.chars().count() <= max_len {
        s.to_owned()
    } else {
        let mut new_s: String = s.to_owned().chars().take(max_len - 1).collect(); // truncate on unicode char boundaries
        new_s.push('…');
        new_s
    }
}

impl fmt::Display for FsInfo {
    /// Output filesystem information
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let term_width = cmp::max(TERM_COLUMNS.load(Ordering::SeqCst), MIN_FS_BAR_LEN + 3);
        let path_max_len = term_width - 1 - MIN_FS_BAR_LEN;

        let pretty_mount_paths: Vec<String> = self
            .mounts
            .iter()
            .map(|x| {
                Ok(ellipsis(
                    x.mount_path
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Unable to decode mount point"))?,
                    path_max_len,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(|_| fmt::Error)?;
        let max_path_len = pretty_mount_paths
            .iter()
            .map(|x| x.chars().count())
            .max()
            .unwrap();

        for (mount_info, pretty_mount_path) in self.mounts.iter().zip(pretty_mount_paths) {
            let fs_usage = mount_info.used_bytes as f32 / mount_info.total_bytes as f32;
            let text_style = if fs_usage >= 0.95 {
                Red.normal()
            } else if fs_usage >= 0.85 {
                Yellow.normal()
            } else {
                Style::new()
            };

            writeln!(
                f,
                "{}{} {}",
                text_style.paint(&pretty_mount_path),
                text_style.paint(" ".repeat(max_path_len - pretty_mount_path.chars().count())),
                get_fs_bar(
                    mount_info,
                    cmp::max(term_width - max_path_len - 1, MIN_FS_BAR_LEN),
                    text_style
                )
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[test]
    #[serial]
    fn test_output_fs_info() {
        TERM_COLUMNS.store(40, Ordering::SeqCst);
        assert_eq!(
            format!(
                "{}",
                FsInfo {
                    mounts: vec![
                        FsMountInfo {
                            mount_path: PathBuf::from("/foo/bar"),
                            used_bytes: 234_560,
                            total_bytes: 7_891_011
                        },
                        FsMountInfo {
                            mount_path: PathBuf::from("/foo/baz"),
                            used_bytes: 2_345_600_000,
                            total_bytes: 7_891_011_000
                        }
                    ]
                },
            ),
            "/foo/bar ▕  \u{1b}[7m\u{1b}[0m229.1 KB / 7.5 MB (3.0%)   ▏\n/foo/baz ▕███\u{1b}[7m2.2 G\u{1b}[0mB / 7.3 GB (29.7%)   ▏\n"
        );
        assert_eq!(
            format!(
                "{}",
                FsInfo {
                    mounts: vec![FsMountInfo {
                        mount_path: PathBuf::from("/0123456789"),
                        used_bytes: 500,
                        total_bytes: 1000
                    },]
                },
            ),
            "/0123456… ▕███\u{1b}[7m500 B / 100\u{1b}[0m0 B (50.0%)   ▏\n"
        );
    }

    #[test]
    fn test_get_fs_bar() {
        assert_eq!(
            get_fs_bar(
                &FsMountInfo{
                    mount_path: PathBuf::from("/foo/bar"),
                    used_bytes: 23456,
                    total_bytes: 7_891_011
                },
                40,
                Red.normal()
            ),
            "\u{1b}[31m▕\u{1b}[0m\u{1b}[31m\u{1b}[0m\u{1b}[31m       \u{1b}[0m\u{1b}[7;31m\u{1b}[0m\u{1b}[31m22.9 KB / 7.5 MB (0.3%)\u{1b}[0m\u{1b}[31m\u{1b}[0m\u{1b}[31m        \u{1b}[0m\u{1b}[31m▏\u{1b}[0m"
        );
        assert_eq!(
            get_fs_bar(
                &FsMountInfo {
                    mount_path: PathBuf::from("/foo/bar"),
                    used_bytes: 0,
                    total_bytes: 7_891_011
                },
                40,
                Style::new()
            ),
            "▕         \u{1b}[7m\u{1b}[0m0 B / 7.5 MB (0.0%)          ▏"
        );
        assert_eq!(
            get_fs_bar(
                &FsMountInfo {
                    mount_path: PathBuf::from("/foo/bar"),
                    used_bytes: 434_560,
                    total_bytes: 7_891_011
                },
                40,
                Style::new()
            ),
            "▕██     \u{1b}[7m\u{1b}[0m424.4 KB / 7.5 MB (5.5%)       ▏"
        );
        assert_eq!(
            get_fs_bar(
                &FsMountInfo {
                    mount_path: PathBuf::from("/foo/bar"),
                    used_bytes: 4_891_011_000,
                    total_bytes: 7_891_011_000
                },
                40,
                Style::new()
            ),
            "▕███████\u{1b}[7m4.6 GB / 7.3 GB \u{1b}[0m(62.0%)        ▏"
        );
        assert_eq!(
            get_fs_bar(
                &FsMountInfo {
                    mount_path: PathBuf::from("/foo/bar"),
                    used_bytes: 4_891_011_000,
                    total_bytes: 7_891_011_000
                },
                30,
                Style::new()
            ),
            "▕██\u{1b}[7m4.6 GB / 7.3 GB\u{1b}[0m (62.0%)   ▏"
        );
        assert_eq!(
            get_fs_bar(
                &FsMountInfo {
                    mount_path: PathBuf::from("/foo/bar"),
                    used_bytes: 4_891_011_000,
                    total_bytes: 7_891_011_000
                },
                50,
                Style::new()
            ),
            "▕████████████\u{1b}[7m4.6 GB / 7.3 GB (\u{1b}[0m62.0%)             ▏"
        );
        assert_eq!(
            get_fs_bar(
                &FsMountInfo {
                    mount_path: PathBuf::from("/foo/bar"),
                    used_bytes: 6_891_011_000_000,
                    total_bytes: 7_891_011_000_000
                },
                40,
                Style::new()
            ),
            "▕███████\u{1b}[7m6.3 TB / 7.2 TB (87.3%)\u{1b}[0m███     ▏"
        );
        assert_eq!(
            get_fs_bar(
                &FsMountInfo {
                    mount_path: PathBuf::from("/foo/bar"),
                    used_bytes: 7_891_011_000_000,
                    total_bytes: 7_891_011_000_000
                },
                40,
                Style::new()
            ),
            "▕███████\u{1b}[7m7.2 TB / 7.2 TB (100.0%)\u{1b}[0m███████▏"
        );
    }

    #[test]
    fn test_ellipsis() {
        assert_eq!(ellipsis("", 3), "…");
        assert_eq!(ellipsis("", 4), "");
        assert_eq!(ellipsis("", 5), "");
    }
}
