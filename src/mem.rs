use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use std::sync::atomic::Ordering;

use ansi_term::Style;

use crate::fmt::format_kmgt;
use crate::module::{ModuleData, TERM_COLUMNS};

pub struct MemInfo {
    /// Map of memory usage info, unit is kB or page count
    vals: HashMap<String, u64>,
}

pub struct SwapInfo {
    mem: MemInfo,
}

impl From<MemInfo> for SwapInfo {
    fn from(mi: MemInfo) -> Self {
        Self { mem: mi }
    }
}

/// Fetch memory usage info from procfs
pub fn fetch() -> anyhow::Result<ModuleData> {
    let mut vals = HashMap::new();
    let file = File::open("/proc/meminfo")?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        // Parse line
        let line_str = line?;
        let mut tokens_it = line_str.split(':');
        let key = tokens_it
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to parse memory info"))?
            .to_string();
        let val_str = tokens_it
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to parse memory value"))?
            .trim_start();
        let val = u64::from_str(
            val_str
                .split(' ')
                .next()
                .ok_or_else(|| anyhow::anyhow!("Failed to parse memory value"))?,
        )?;

        // Store info
        vals.insert(key, val);
    }

    Ok(ModuleData::Memory(MemInfo { vals }))
}

/// Memory bar section
struct BarPart {
    /// Section text
    label: Vec<String>,
    /// Percentage of full bar this section should fill
    prct: f32,
    /// Bar text style
    text_style: Style,
    /// Bar fill char style
    fill_style: Style,
    /// Char to use to fill bar
    bar_char: char,
}

/// Print memory bar
fn display_bar(parts: &[BarPart], f: &mut dyn fmt::Write) -> fmt::Result {
    // Compute part lengths and handle rounding
    let term_columns = TERM_COLUMNS.load(Ordering::SeqCst);
    let mut part_lens_int: Vec<usize> = parts
        .iter()
        .map(|part| ((term_columns - 2) as f32 * part.prct / 100.0) as usize)
        .collect();
    while &part_lens_int.iter().sum() + (2_usize) < term_columns {
        // Compute fractional parts
        let part_lens_frac: Vec<f32> = parts
            .iter()
            .zip(&part_lens_int)
            .map(|(part, &part_len_int)| {
                f32::max(
                    0.0,
                    ((term_columns - 2) as f32 * part.prct / 100.0) - part_len_int as f32,
                )
            })
            .collect();

        // Find part_lens_frac first maximum, add 1 to corresponding integer part
        *part_lens_frac
            .iter()
            .zip(&mut part_lens_int)
            .rev() // max_by gets last maximum, this allows getting the first
            .max_by(|(a_frac, _a_int), (b_frac, _b_int)| a_frac.partial_cmp(b_frac).unwrap())
            .unwrap()
            .1 += 1;
    }

    write!(f, "▕")?;

    for (part, part_len) in parts.iter().zip(part_lens_int) {
        // Build longest label that fits
        let mut label = String::new();
        for label_part in &part.label {
            if label.len() + label_part.len() <= part_len {
                label += label_part;
            } else {
                break;
            }
        }

        // Center bar text inside fill chars
        let label_len = label.len();
        let fill_count_before = (part_len - label_len) / 2;
        let fill_count_after = if (part_len - label_len) % 2 == 1 {
            fill_count_before + 1
        } else {
            fill_count_before
        };
        write!(
            f,
            "{}",
            &part
                .fill_style
                .paint(part.bar_char.to_string().repeat(fill_count_before))
        )?;
        write!(f, "{}", &part.text_style.paint(&label))?;
        write!(
            f,
            "{}",
            &part
                .fill_style
                .paint(part.bar_char.to_string().repeat(fill_count_after))
        )?;
    }

    writeln!(f, "▏")?;

    Ok(())
}

impl MemInfo {
    /// Print memory stat numbers
    fn display_stats(
        &self,
        keys: Vec<&str>,
        total_key: &str,
        f: &mut dyn fmt::Write,
    ) -> fmt::Result {
        let max_key_len = keys.iter().map(|x| x.len()).max().unwrap();
        let mac_size_str_len = keys
            .iter()
            .map(|&x| format_kmgt(self.vals[x] * 1024, "B").len())
            .max()
            .unwrap();

        for &key in keys.iter() {
            let size_str = format_kmgt(self.vals[key] * 1024, "B");
            write!(
                f,
                "{}: {}{}",
                key,
                " ".repeat(max_key_len - key.len() + mac_size_str_len - size_str.len()),
                size_str
            )?;
            if key != total_key {
                write!(
                    f,
                    " ({: >4.1}%)",
                    100.0 * self.vals[key] as f32 / self.vals[total_key] as f32
                )?;
            }

            writeln!(f)?;
        }

        Ok(())
    }
}

impl fmt::Display for MemInfo {
    /// Output memory info
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.display_stats(
            vec!["MemTotal", "MemFree", "Dirty", "Cached", "Buffers"],
            "MemTotal",
            f,
        )?;

        let total_mem_mb = self.vals["MemTotal"] / 1024;
        let cache_mem_mb = self.vals["Cached"] / 1024;
        let buffer_mem_mb = self.vals["Buffers"] / 1024;
        let free_mem_mb = self.vals["MemFree"] / 1024;
        let used_mem_mb = total_mem_mb - cache_mem_mb - buffer_mem_mb - free_mem_mb;

        let mut mem_bar_parts = Vec::new();

        let used_prct = 100.0 * used_mem_mb as f32 / total_mem_mb as f32;
        let used_bar_text: Vec<String> = vec![
            "Used".to_string(),
            format!(" {:.1}GB", used_mem_mb as f32 / 1024.0),
            format!(" ({used_prct:.1}%)"),
        ];
        mem_bar_parts.push(BarPart {
            label: used_bar_text,
            prct: used_prct,
            text_style: Style::new().reverse(),
            fill_style: Style::new(),
            bar_char: '█',
        });

        let cached_prct = 100.0 * (cache_mem_mb + buffer_mem_mb) as f32 / total_mem_mb as f32;
        let cached_bar_text: Vec<String> = vec![
            "Cached".to_string(),
            format!(" {:.1}GB", (cache_mem_mb + buffer_mem_mb) as f32 / 1024.0),
            format!(" ({cached_prct:.1}%)"),
        ];
        mem_bar_parts.push(BarPart {
            label: cached_bar_text,
            prct: cached_prct,
            text_style: Style::new().dimmed().reverse(),
            fill_style: Style::new().dimmed(),
            bar_char: '█',
        });

        let free_prct = 100.0 * free_mem_mb as f32 / total_mem_mb as f32;
        let free_bar_text: Vec<String> = vec![
            "Free".to_string(),
            format!(" {:.1}GB", free_mem_mb as f32 / 1024.0),
            format!(" ({free_prct:.1}%)"),
        ];
        mem_bar_parts.push(BarPart {
            label: free_bar_text,
            prct: free_prct,
            text_style: Style::new(),
            fill_style: Style::new(),
            bar_char: ' ',
        });

        display_bar(&mem_bar_parts, f)?;

        Ok(())
    }
}

impl fmt::Display for SwapInfo {
    /// Output swap info
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.mem.vals["SwapTotal"] > 0 {
            self.mem
                .display_stats(vec!["SwapTotal", "SwapFree"], "SwapTotal", f)?;

            let total_swap_mb = self.mem.vals["SwapTotal"] / 1024;
            let free_swap_mb = self.mem.vals["SwapFree"] / 1024;
            let used_swap_mb = total_swap_mb - free_swap_mb;

            let mut swap_bar_parts = Vec::new();

            let used_prct = 100.0 * used_swap_mb as f32 / total_swap_mb as f32;
            let used_bar_text: Vec<String> = vec![
                "Used".to_string(),
                format!(" {:.1}GB", used_swap_mb as f32 / 1024.0),
                format!(" ({used_prct:.1}%)"),
            ];
            swap_bar_parts.push(BarPart {
                label: used_bar_text,
                prct: used_prct,
                text_style: Style::new().reverse(),
                fill_style: Style::new(),
                bar_char: '█',
            });

            let free_prct = 100.0 * free_swap_mb as f32 / total_swap_mb as f32;
            let free_bar_text: Vec<String> = vec![
                "Swap free".to_string(),
                format!(" {:.1}GB", free_swap_mb as f32 / 1024.0),
                format!(" ({free_prct:.1}%)"),
            ];
            swap_bar_parts.push(BarPart {
                label: free_bar_text,
                prct: free_prct,
                text_style: Style::new(),
                fill_style: Style::new(),
                bar_char: ' ',
            });

            display_bar(&swap_bar_parts, f)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::module;
    use ansi_term::Colour::*;

    use serial_test::serial;

    #[test]
    #[serial]
    fn test_output_bar() {
        // Check rounding
        module::TERM_COLUMNS.store(102, Ordering::SeqCst);
        let mut f = String::new();
        display_bar(
            &[
                BarPart {
                    label: vec![
                        "part1".to_string(),
                        "PART1".to_string(),
                        "P_A_R_T_1".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '#',
                },
                BarPart {
                    label: vec![
                        "part2".to_string(),
                        "PART2".to_string(),
                        "P_A_R_T_2".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: 'X',
                },
                BarPart {
                    label: vec![
                        "part3".to_string(),
                        "PART3".to_string(),
                        "P_A_R_T_3".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '%',
                },
            ],
            &mut f,
        )
        .unwrap();
        assert_eq!(
            f,
            "▕#######part1PART1P_A_R_T_1########XXXXXXXpart2PART2P_A_R_T_2XXXXXXX%%%%%%%part3PART3P_A_R_T_3%%%%%%%▏\n"
        );

        f.clear();
        display_bar(
            &[
                BarPart {
                    label: vec![
                        "part1".to_string(),
                        "PART1".to_string(),
                        "P_A_R_T_1".to_string(),
                    ],
                    prct: 20.34,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '#',
                },
                BarPart {
                    label: vec![
                        "part2".to_string(),
                        "PART2".to_string(),
                        "P_A_R_T_2".to_string(),
                    ],
                    prct: 30.32,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: 'X',
                },
                BarPart {
                    label: vec![
                        "part3".to_string(),
                        "PART3".to_string(),
                        "P_A_R_T_3".to_string(),
                    ],
                    prct: 48.33,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '%',
                },
            ],
            &mut f,
        )
        .unwrap();
        assert_eq!(
            f,
            "▕#part1PART1P_A_R_T_1#XXXXXpart2PART2P_A_R_T_2XXXXXX%%%%%%%%%%%%%%%part3PART3P_A_R_T_3%%%%%%%%%%%%%%%▏\n"
        );

        f.clear();
        display_bar(
            &[
                BarPart {
                    label: vec![
                        "part1".to_string(),
                        "PART1".to_string(),
                        "P_A_R_T_1".to_string(),
                    ],
                    prct: 20.5,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '#',
                },
                BarPart {
                    label: vec![
                        "part2".to_string(),
                        "PART2".to_string(),
                        "P_A_R_T_2".to_string(),
                    ],
                    prct: 30.6,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: 'X',
                },
                BarPart {
                    label: vec![
                        "part3".to_string(),
                        "PART3".to_string(),
                        "P_A_R_T_3".to_string(),
                    ],
                    prct: 48.9,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '%',
                },
            ],
            &mut f,
        )
        .unwrap();
        assert_eq!(
            f,
            "▕part1PART1P_A_R_T_1#XXXXXXpart2PART2P_A_R_T_2XXXXXX%%%%%%%%%%%%%%%part3PART3P_A_R_T_3%%%%%%%%%%%%%%%▏\n"
        );

        module::TERM_COLUMNS.store(80, Ordering::SeqCst);
        f.clear();
        display_bar(
            &[
                BarPart {
                    label: vec![
                        "part1".to_string(),
                        "PART1".to_string(),
                        "P_A_R_T_1".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '#',
                },
                BarPart {
                    label: vec![
                        "part2".to_string(),
                        "PART2".to_string(),
                        "P_A_R_T_2".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: 'X',
                },
                BarPart {
                    label: vec![
                        "part3".to_string(),
                        "PART3".to_string(),
                        "P_A_R_T_3".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '%',
                },
            ],
            &mut f,
        )
        .unwrap();
        assert_eq!(
            f,
            "▕###part1PART1P_A_R_T_1####XXXpart2PART2P_A_R_T_2XXXX%%%part3PART3P_A_R_T_3%%%%▏\n"
        );

        module::TERM_COLUMNS.store(50, Ordering::SeqCst);
        f.clear();
        display_bar(
            &[
                BarPart {
                    label: vec![
                        "part1".to_string(),
                        "PART1".to_string(),
                        "P_A_R_T_1".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '#',
                },
                BarPart {
                    label: vec![
                        "part2".to_string(),
                        "PART2".to_string(),
                        "P_A_R_T_2".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: 'X',
                },
                BarPart {
                    label: vec![
                        "part3".to_string(),
                        "PART3".to_string(),
                        "P_A_R_T_3".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '%',
                },
            ],
            &mut f,
        )
        .unwrap();
        assert_eq!(f, "▕###part1PART1###XXXpart2PART2XXX%%%part3PART3%%%▏\n");

        module::TERM_COLUMNS.store(30, Ordering::SeqCst);
        f.clear();
        display_bar(
            &[
                BarPart {
                    label: vec![
                        "part1".to_string(),
                        "PART1".to_string(),
                        "P_A_R_T_1".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '#',
                },
                BarPart {
                    label: vec![
                        "part2".to_string(),
                        "PART2".to_string(),
                        "P_A_R_T_2".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: 'X',
                },
                BarPart {
                    label: vec![
                        "part3".to_string(),
                        "PART3".to_string(),
                        "P_A_R_T_3".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '%',
                },
            ],
            &mut f,
        )
        .unwrap();
        assert_eq!(f, "▕part1PART1XXpart2XX%%part3%%▏\n");

        module::TERM_COLUMNS.store(15, Ordering::SeqCst);
        f.clear();
        display_bar(
            &[
                BarPart {
                    label: vec![
                        "part1".to_string(),
                        "PART1".to_string(),
                        "P_A_R_T_1".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '#',
                },
                BarPart {
                    label: vec![
                        "part2".to_string(),
                        "PART2".to_string(),
                        "P_A_R_T_2".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: 'X',
                },
                BarPart {
                    label: vec![
                        "part3".to_string(),
                        "PART3".to_string(),
                        "P_A_R_T_3".to_string(),
                    ],
                    prct: 100.0 / 3.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '%',
                },
            ],
            &mut f,
        )
        .unwrap();
        assert_eq!(f, "▕part1XXXX%%%%▏\n");

        module::TERM_COLUMNS.store(50, Ordering::SeqCst);
        f.clear();
        display_bar(
            &[
                BarPart {
                    label: vec![
                        "part1".to_string(),
                        "PART1".to_string(),
                        "P_A_R_T_1".to_string(),
                    ],
                    prct: 15.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '#',
                },
                BarPart {
                    label: vec![
                        "part2".to_string(),
                        "PART2".to_string(),
                        "P_A_R_T_2".to_string(),
                    ],
                    prct: 20.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: 'X',
                },
                BarPart {
                    label: vec![
                        "part3".to_string(),
                        "PART3".to_string(),
                        "P_A_R_T_3".to_string(),
                    ],
                    prct: 65.0,
                    text_style: Style::new(),
                    fill_style: Style::new(),
                    bar_char: '%',
                },
            ],
            &mut f,
        )
        .unwrap();
        assert_eq!(f, "▕#part1#part2PART2%%%%%%part3PART3P_A_R_T_3%%%%%%▏\n");

        f.clear();
        display_bar(
            &[
                BarPart {
                    label: vec![
                        "part1".to_string(),
                        "PART1".to_string(),
                        "P_A_R_T_1".to_string(),
                    ],
                    prct: 15.0,
                    text_style: Red.bold(),
                    fill_style: Red.underline(),
                    bar_char: '#',
                },
                BarPart {
                    label: vec![
                        "part2".to_string(),
                        "PART2".to_string(),
                        "P_A_R_T_2".to_string(),
                    ],
                    prct: 20.0,
                    text_style: Yellow.dimmed(),
                    fill_style: Yellow.italic(),
                    bar_char: 'X',
                },
                BarPart {
                    label: vec![
                        "part3".to_string(),
                        "PART3".to_string(),
                        "P_A_R_T_3".to_string(),
                    ],
                    prct: 65.0,
                    text_style: Blue.reverse(),
                    fill_style: Blue.blink(),
                    bar_char: '%',
                },
            ],
            &mut f,
        )
        .unwrap();
        assert_eq!(
            f,
            "▕\u{1b}[4;31m#\u{1b}[0m\u{1b}[1;31mpart1\u{1b}[0m\u{1b}[4;31m#\u{1b}[0m\u{1b}[3;33m\u{1b}[0m\u{1b}[2;33mpart2PART2\u{1b}[0m\u{1b}[3;33m\u{1b}[0m\u{1b}[5;34m%%%%%%\u{1b}[0m\u{1b}[7;34mpart3PART3P_A_R_T_3\u{1b}[0m\u{1b}[5;34m%%%%%%\u{1b}[0m▏\n"
        );
    }

    #[test]
    fn test_output_mem_stats() {
        let mut vals = HashMap::new();
        vals.insert("stat1".to_string(), 123);
        vals.insert("stat22222222".to_string(), 1234567);
        vals.insert("stat3333".to_string(), 123456789);
        vals.insert("itsatrap".to_string(), 999);
        let mem_info = MemInfo { vals };

        let mut f = String::new();
        mem_info
            .display_stats(
                vec!["stat1", "stat22222222", "stat3333"],
                "stat3333",
                &mut f,
            )
            .unwrap();
        assert_eq!(
            f,
            "stat1:        123.0 KB ( 0.0%)\nstat22222222:   1.2 GB ( 1.0%)\nstat3333:     117.7 GB\n"
        );
    }

    #[test]
    #[serial]
    fn test_output_mem() {
        let mut vals = HashMap::new();
        vals.insert("MemTotal".to_string(), 12345);
        vals.insert("MemFree".to_string(), 1234);
        vals.insert("Dirty".to_string(), 2134);
        vals.insert("Cached".to_string(), 3124);
        vals.insert("Buffers".to_string(), 4321);
        vals.insert("itsatrap".to_string(), 1024);
        let mem_info = MemInfo { vals };

        module::TERM_COLUMNS.store(80, Ordering::SeqCst);
        assert_eq!(
            format!("{}", &mem_info),
            "MemTotal: 12.1 MB\nMemFree:   1.2 MB (10.0%)\nDirty:     2.1 MB (17.3%)\nCached:    3.1 MB (25.3%)\nBuffers:   4.2 MB (35.0%)\n▕████\u{1b}[7mUsed 0.0GB (33.3%)\u{1b}[0m████\u{1b}[2m█████████████\u{1b}[0m\u{1b}[2;7mCached 0.0GB (58.3%)\u{1b}[0m\u{1b}[2m█████████████\u{1b}[0m Free ▏\n"
        );

        module::TERM_COLUMNS.store(30, Ordering::SeqCst);
        assert_eq!(
            format!("{}", &mem_info),
            "MemTotal: 12.1 MB\nMemFree:   1.2 MB (10.0%)\nDirty:     2.1 MB (17.3%)\nCached:    3.1 MB (25.3%)\nBuffers:   4.2 MB (35.0%)\n▕██\u{1b}[7mUsed\u{1b}[0m███\u{1b}[2m██\u{1b}[0m\u{1b}[2;7mCached 0.0GB\u{1b}[0m\u{1b}[2m██\u{1b}[0m   ▏\n"
        );
    }

    #[test]
    #[serial]
    fn test_output_swap() {
        let mut vals = HashMap::new();
        vals.insert("SwapTotal".to_string(), 12345678);
        vals.insert("SwapFree".to_string(), 2345678);
        vals.insert("itsatrap".to_string(), 1024);
        let mem_info = MemInfo { vals };
        let swap_info = SwapInfo::from(mem_info);

        module::TERM_COLUMNS.store(80, Ordering::SeqCst);
        assert_eq!(
            format!("{}", &swap_info),
            "SwapTotal: 11.8 GB\nSwapFree:   2.2 GB (19.0%)\n▕██████████████████████\u{1b}[7mUsed 9.5GB (81.0%)\u{1b}[0m███████████████████████Swap free 2.2GB▏\n"
        );

        module::TERM_COLUMNS.store(30, Ordering::SeqCst);
        assert_eq!(
            format!("{}", &swap_info),
            "SwapTotal: 11.8 GB\nSwapFree:   2.2 GB (19.0%)\n▕██\u{1b}[7mUsed 9.5GB (81.0%)\u{1b}[0m███     ▏\n"
        );

        let mut vals = HashMap::new();
        vals.insert("SwapTotal".to_string(), 0);
        vals.insert("SwapFree".to_string(), 0);
        vals.insert("itsatrap".to_string(), 1024);
        let mem_info = MemInfo { vals };
        let swap_info = SwapInfo::from(mem_info);

        assert!(format!("{}", &swap_info).is_empty());
    }
}
