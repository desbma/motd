use std::collections::HashMap;
use std::error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;

use ansi_term::Style;
use simple_error::SimpleError;

use crate::fmt::format_kmg;

/// Map of memory usage info, unit is kB or page count
pub type MemInfo = HashMap<String, u64>;

/// Fetch memory usage info from procfs
pub fn get_mem_info() -> Result<MemInfo, Box<dyn error::Error>> {
    let mut mem_info = MemInfo::new();
    let file = File::open("/proc/meminfo")?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        // Parse line
        let line_str = line?;
        let mut tokens_it = line_str.split(':');
        let key = tokens_it
            .next()
            .ok_or_else(|| SimpleError::new("Failed to parse memory info"))?
            .to_string();
        let val_str = tokens_it
            .next()
            .ok_or_else(|| SimpleError::new("Failed to parse memory value"))?
            .trim_start();
        let val = u64::from_str(
            val_str
                .split(' ')
                .next()
                .ok_or_else(|| SimpleError::new("Failed to parse memory value"))?,
        )?;

        // Store info
        mem_info.insert(key, val);
    }

    Ok(mem_info)
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
fn output_bar(parts: &[BarPart], length: usize) -> String {
    // Compute part lengths and handle rounding
    let mut part_lens_int: Vec<usize> = parts
        .iter()
        .map(|part| ((length - 2) as f32 * part.prct / 100.0) as usize)
        .collect();
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::op_ref))]
    while &part_lens_int.iter().sum() + (2_usize) < length {
        // Compute fractional parts
        let part_lens_frac: Vec<f32> = parts
            .iter()
            .zip(&part_lens_int)
            .map(|(part, &part_len_int)| {
                f32::max(
                    0.0,
                    ((length - 2) as f32 * part.prct / 100.0) - part_len_int as f32,
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

    let mut full_bar = "▕".to_string();
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
        full_bar += &part
            .fill_style
            .paint(&part.bar_char.to_string().repeat(fill_count_before))
            .to_string();
        full_bar += &part.text_style.paint(&label).to_string();
        full_bar += &part
            .fill_style
            .paint(&part.bar_char.to_string().repeat(fill_count_after))
            .to_string();
    }

    full_bar.push('▏');

    full_bar
}

/// Print memory stat numbers
fn output_mem_stats(mem_info: &MemInfo, keys: Vec<&str>, total_key: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    let max_key_len = keys.iter().map(|x| x.len()).max().unwrap();
    let mac_size_str_len = keys
        .iter()
        .map(|&x| format_kmg(mem_info[x] * 1024, "B").len())
        .max()
        .unwrap();
    for &key in keys.iter() {
        let size_str = format_kmg(mem_info[key] * 1024, "B");
        let mut line: String = format!(
            "{}: {}{}",
            key,
            " ".repeat(max_key_len - key.len() + mac_size_str_len - size_str.len()),
            size_str
        );
        if key != total_key {
            line += &format!(
                " ({: >4.1}%)",
                100.0 * mem_info[key] as f32 / mem_info[total_key] as f32
            );
        }
        lines.push(line);
    }

    lines
}

/// Output memory info
pub fn output_mem(mem_info: &MemInfo, term_width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    lines.extend(output_mem_stats(
        mem_info,
        vec!["MemTotal", "MemFree", "Dirty", "Cached", "Buffers"],
        "MemTotal",
    ));

    let total_mem_mb = mem_info["MemTotal"] / 1024;
    let cache_mem_mb = mem_info["Cached"] / 1024;
    let buffer_mem_mb = mem_info["Buffers"] / 1024;
    let free_mem_mb = mem_info["MemFree"] / 1024;
    let used_mem_mb = total_mem_mb - cache_mem_mb - buffer_mem_mb - free_mem_mb;

    let mut mem_bar_parts = Vec::new();

    let used_prct = 100.0 * used_mem_mb as f32 / total_mem_mb as f32;
    let used_bar_text: Vec<String> = vec![
        "Used".to_string(),
        format!(" {:.1}GB", used_mem_mb as f32 / 1024.0),
        format!(" ({:.1}%)", used_prct),
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
        format!(" ({:.1}%)", cached_prct),
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
        format!(" ({:.1}%)", free_prct),
    ];
    mem_bar_parts.push(BarPart {
        label: free_bar_text,
        prct: free_prct,
        text_style: Style::new(),
        fill_style: Style::new(),
        bar_char: ' ',
    });

    lines.push(output_bar(&mem_bar_parts, term_width));

    lines
}

/// Output swap info
pub fn output_swap(mem_info: &MemInfo, term_width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    if mem_info["SwapTotal"] > 0 {
        lines.extend(output_mem_stats(
            mem_info,
            vec!["SwapTotal", "SwapFree"],
            "SwapTotal",
        ));

        let total_swap_mb = mem_info["SwapTotal"] / 1024;
        let free_swap_mb = mem_info["SwapFree"] / 1024;
        let used_swap_mb = total_swap_mb - free_swap_mb;

        let mut swap_bar_parts = Vec::new();

        let used_prct = 100.0 * used_swap_mb as f32 / total_swap_mb as f32;
        let used_bar_text: Vec<String> = vec![
            "Used".to_string(),
            format!(" {:.1}GB", used_swap_mb as f32 / 1024.0),
            format!(" ({:.1}%)", used_prct),
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
            format!(" ({:.1}%)", free_prct),
        ];
        swap_bar_parts.push(BarPart {
            label: free_bar_text,
            prct: free_prct,
            text_style: Style::new(),
            fill_style: Style::new(),
            bar_char: ' ',
        });

        lines.push(output_bar(&swap_bar_parts, term_width));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    use ansi_term::Colour::*;

    #[test]
    fn test_output_bar() {
        // Check rounding
        assert_eq!(
            output_bar(
                &[
                    BarPart{
                        label: vec![
                            "part1".to_string(),
                            "PART1".to_string(),
                            "P_A_R_T_1".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '#'
                    },
                    BarPart{
                        label: vec![
                            "part2".to_string(),
                            "PART2".to_string(),
                            "P_A_R_T_2".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: 'X'
                    },
                    BarPart{
                        label: vec![
                            "part3".to_string(),
                            "PART3".to_string(),
                            "P_A_R_T_3".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '%'
                    }
                ],
                102
            ),
            "▕#######part1PART1P_A_R_T_1########XXXXXXXpart2PART2P_A_R_T_2XXXXXXX%%%%%%%part3PART3P_A_R_T_3%%%%%%%▏"
        );
        assert_eq!(
            output_bar(
                &[
                    BarPart{
                        label: vec![
                            "part1".to_string(),
                            "PART1".to_string(),
                            "P_A_R_T_1".to_string()
                        ],
                        prct: 20.34,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '#'
                    },
                    BarPart{
                        label: vec![
                            "part2".to_string(),
                            "PART2".to_string(),
                            "P_A_R_T_2".to_string()
                        ],
                        prct: 30.32,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: 'X'
                    },
                    BarPart{
                        label: vec![
                            "part3".to_string(),
                            "PART3".to_string(),
                            "P_A_R_T_3".to_string()
                        ],
                        prct: 48.33,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '%'
                    }
                ],
                102
            ),
            "▕#part1PART1P_A_R_T_1#XXXXXpart2PART2P_A_R_T_2XXXXXX%%%%%%%%%%%%%%%part3PART3P_A_R_T_3%%%%%%%%%%%%%%%▏"
        );
        assert_eq!(
            output_bar(
                &[
                    BarPart{
                        label: vec![
                            "part1".to_string(),
                            "PART1".to_string(),
                            "P_A_R_T_1".to_string()
                        ],
                        prct: 20.5,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '#'
                    },
                    BarPart{
                        label: vec![
                            "part2".to_string(),
                            "PART2".to_string(),
                            "P_A_R_T_2".to_string()
                        ],
                        prct: 30.6,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: 'X'
                    },
                    BarPart{
                        label: vec![
                            "part3".to_string(),
                            "PART3".to_string(),
                            "P_A_R_T_3".to_string()
                        ],
                        prct: 48.9,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '%'
                    }
                ],
                102
            ),
            "▕part1PART1P_A_R_T_1#XXXXXXpart2PART2P_A_R_T_2XXXXXX%%%%%%%%%%%%%%%part3PART3P_A_R_T_3%%%%%%%%%%%%%%%▏"
        );

        assert_eq!(
            output_bar(
                &[
                    BarPart {
                        label: vec![
                            "part1".to_string(),
                            "PART1".to_string(),
                            "P_A_R_T_1".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '#'
                    },
                    BarPart {
                        label: vec![
                            "part2".to_string(),
                            "PART2".to_string(),
                            "P_A_R_T_2".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: 'X'
                    },
                    BarPart {
                        label: vec![
                            "part3".to_string(),
                            "PART3".to_string(),
                            "P_A_R_T_3".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '%'
                    }
                ],
                80
            ),
            "▕###part1PART1P_A_R_T_1####XXXpart2PART2P_A_R_T_2XXXX%%%part3PART3P_A_R_T_3%%%%▏"
        );
        assert_eq!(
            output_bar(
                &[
                    BarPart {
                        label: vec![
                            "part1".to_string(),
                            "PART1".to_string(),
                            "P_A_R_T_1".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '#'
                    },
                    BarPart {
                        label: vec![
                            "part2".to_string(),
                            "PART2".to_string(),
                            "P_A_R_T_2".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: 'X'
                    },
                    BarPart {
                        label: vec![
                            "part3".to_string(),
                            "PART3".to_string(),
                            "P_A_R_T_3".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '%'
                    }
                ],
                50
            ),
            "▕###part1PART1###XXXpart2PART2XXX%%%part3PART3%%%▏"
        );
        assert_eq!(
            output_bar(
                &[
                    BarPart {
                        label: vec![
                            "part1".to_string(),
                            "PART1".to_string(),
                            "P_A_R_T_1".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '#'
                    },
                    BarPart {
                        label: vec![
                            "part2".to_string(),
                            "PART2".to_string(),
                            "P_A_R_T_2".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: 'X'
                    },
                    BarPart {
                        label: vec![
                            "part3".to_string(),
                            "PART3".to_string(),
                            "P_A_R_T_3".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '%'
                    }
                ],
                30
            ),
            "▕part1PART1XXpart2XX%%part3%%▏"
        );
        assert_eq!(
            output_bar(
                &[
                    BarPart {
                        label: vec![
                            "part1".to_string(),
                            "PART1".to_string(),
                            "P_A_R_T_1".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '#'
                    },
                    BarPart {
                        label: vec![
                            "part2".to_string(),
                            "PART2".to_string(),
                            "P_A_R_T_2".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: 'X'
                    },
                    BarPart {
                        label: vec![
                            "part3".to_string(),
                            "PART3".to_string(),
                            "P_A_R_T_3".to_string()
                        ],
                        prct: 100.0 / 3.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '%'
                    }
                ],
                15
            ),
            "▕part1XXXX%%%%▏"
        );
        assert_eq!(
            output_bar(
                &[
                    BarPart {
                        label: vec![
                            "part1".to_string(),
                            "PART1".to_string(),
                            "P_A_R_T_1".to_string()
                        ],
                        prct: 15.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '#'
                    },
                    BarPart {
                        label: vec![
                            "part2".to_string(),
                            "PART2".to_string(),
                            "P_A_R_T_2".to_string()
                        ],
                        prct: 20.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: 'X'
                    },
                    BarPart {
                        label: vec![
                            "part3".to_string(),
                            "PART3".to_string(),
                            "P_A_R_T_3".to_string()
                        ],
                        prct: 65.0,
                        text_style: Style::new(),
                        fill_style: Style::new(),
                        bar_char: '%'
                    }
                ],
                50
            ),
            "▕#part1#part2PART2%%%%%%part3PART3P_A_R_T_3%%%%%%▏"
        );
        assert_eq!(
            output_bar(
                &[
                    BarPart{
                        label: vec![
                            "part1".to_string(),
                            "PART1".to_string(),
                            "P_A_R_T_1".to_string()
                        ],
                        prct: 15.0,
                        text_style: Red.bold(),
                        fill_style: Red.underline(),
                        bar_char: '#'
                    },
                    BarPart{
                        label: vec![
                            "part2".to_string(),
                            "PART2".to_string(),
                            "P_A_R_T_2".to_string()
                        ],
                        prct: 20.0,
                        text_style: Yellow.dimmed(),
                        fill_style: Yellow.italic(),
                        bar_char: 'X'
                    },
                    BarPart{
                        label: vec![
                            "part3".to_string(),
                            "PART3".to_string(),
                            "P_A_R_T_3".to_string()
                        ],
                        prct: 65.0,
                        text_style: Blue.reverse(),
                        fill_style: Blue.blink(),
                        bar_char: '%'
                    }
                ],
                50
            ),
            "▕\u{1b}[4;31m#\u{1b}[0m\u{1b}[1;31mpart1\u{1b}[0m\u{1b}[4;31m#\u{1b}[0m\u{1b}[3;33m\u{1b}[0m\u{1b}[2;33mpart2PART2\u{1b}[0m\u{1b}[3;33m\u{1b}[0m\u{1b}[5;34m%%%%%%\u{1b}[0m\u{1b}[7;34mpart3PART3P_A_R_T_3\u{1b}[0m\u{1b}[5;34m%%%%%%\u{1b}[0m▏"
        );
    }

    #[test]
    fn test_output_mem_stats() {
        let mut mem_stats = MemInfo::new();
        mem_stats.insert("stat1".to_string(), 123);
        mem_stats.insert("stat22222222".to_string(), 1234567);
        mem_stats.insert("stat3333".to_string(), 123456789);
        mem_stats.insert("itsatrap".to_string(), 999);
        assert_eq!(
            output_mem_stats(
                &mem_stats,
                vec!["stat1", "stat22222222", "stat3333"],
                "stat3333"
            ),
            [
                "stat1:        123.00 KB ( 0.0%)",
                "stat22222222:   1.18 GB ( 1.0%)",
                "stat3333:     117.74 GB"
            ]
        );
    }

    #[test]
    fn test_output_mem() {
        let mut mem_stats = MemInfo::new();
        mem_stats.insert("MemTotal".to_string(), 12345);
        mem_stats.insert("MemFree".to_string(), 1234);
        mem_stats.insert("Dirty".to_string(), 2134);
        mem_stats.insert("Cached".to_string(), 3124);
        mem_stats.insert("Buffers".to_string(), 4321);
        mem_stats.insert("itsatrap".to_string(), 1024);
        assert_eq!(
            output_mem(&mem_stats, 80),
            ["MemTotal: 12.06 MB", "MemFree:   1.21 MB (10.0%)", "Dirty:     2.08 MB (17.3%)", "Cached:    3.05 MB (25.3%)", "Buffers:   4.22 MB (35.0%)", "▕████\u{1b}[7mUsed 0.0GB (33.3%)\u{1b}[0m████\u{1b}[2m█████████████\u{1b}[0m\u{1b}[2;7mCached 0.0GB (58.3%)\u{1b}[0m\u{1b}[2m█████████████\u{1b}[0m Free ▏"]
        );
        assert_eq!(
            output_mem(&mem_stats, 30),
            ["MemTotal: 12.06 MB", "MemFree:   1.21 MB (10.0%)", "Dirty:     2.08 MB (17.3%)", "Cached:    3.05 MB (25.3%)", "Buffers:   4.22 MB (35.0%)", "▕██\u{1b}[7mUsed\u{1b}[0m███\u{1b}[2m██\u{1b}[0m\u{1b}[2;7mCached 0.0GB\u{1b}[0m\u{1b}[2m██\u{1b}[0m   ▏"]
        );
    }

    #[test]
    fn test_output_swap() {
        let mut mem_stats = MemInfo::new();
        mem_stats.insert("SwapTotal".to_string(), 12345678);
        mem_stats.insert("SwapFree".to_string(), 2345678);
        mem_stats.insert("itsatrap".to_string(), 1024);
        assert_eq!(
            output_swap(&mem_stats, 80),
            ["SwapTotal: 11.77 GB", "SwapFree:   2.24 GB (19.0%)", "▕██████████████████████\u{1b}[7mUsed 9.5GB (81.0%)\u{1b}[0m███████████████████████Swap free 2.2GB▏"]
        );
        assert_eq!(
            output_swap(&mem_stats, 30),
            [
                "SwapTotal: 11.77 GB",
                "SwapFree:   2.24 GB (19.0%)",
                "▕██\u{1b}[7mUsed 9.5GB (81.0%)\u{1b}[0m███     ▏"
            ]
        );

        let mut mem_stats = MemInfo::new();
        mem_stats.insert("SwapTotal".to_string(), 0);
        mem_stats.insert("SwapFree".to_string(), 0);
        mem_stats.insert("itsatrap".to_string(), 1024);
        assert!(output_swap(&mem_stats, 80).is_empty());
    }
}
