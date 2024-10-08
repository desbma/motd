use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, DirEntry, File},
    io::{Read, Seek},
    thread::sleep,
    time::{Duration, Instant},
};

use ansi_term::Colour::{Red, Yellow};

use crate::{fmt::format_kmgt_si, module::ModuleData};

/// Network interface pending stats
struct PendingInterfaceStats {
    /// Rx byte count
    rx_bytes: u64,
    /// Tx byte count
    tx_bytes: u64,
    /// Rx bytes count sysfs file
    rx_bytes_file: File,
    /// Tx bytes count sysfs file
    tx_bytes_file: File,
    /// Timestamp
    ts: Instant,
    /// Interface speed
    line_bps: Option<u64>,
}

type NetworkPendingStats = BTreeMap<String, PendingInterfaceStats>;

/// Network interface stats
#[expect(clippy::struct_field_names)]
pub(crate) struct InterfaceStats {
    /// Rx bits/s
    rx_bps: u64,
    /// Tx bits/s
    tx_bps: u64,
    /// Interface speed
    line_bps: Option<u64>,
}

pub(crate) struct NetworkStats {
    interfaces: BTreeMap<String, InterfaceStats>,
}

const MIN_DELAY_BETWEEN_NET_SAMPLES_MS: u64 = 30;

pub(crate) fn fetch() -> anyhow::Result<ModuleData> {
    let mut sample = get_network_stats()?;
    let stats = update_network_stats(&mut sample)?;
    Ok(ModuleData::Network(stats))
}

#[expect(clippy::verbose_file_reads)]
fn read_interface_stats(
    rx_bytes_file: &mut File,
    tx_bytes_file: &mut File,
) -> anyhow::Result<(u64, u64, Instant)> {
    let mut rx_str = String::new();
    rx_bytes_file.read_to_string(&mut rx_str)?;
    let rx_bytes = rx_str.trim_end().parse::<u64>()?;

    let mut tx_str = String::new();
    tx_bytes_file.read_to_string(&mut tx_str)?;
    let tx_bytes = tx_str.trim_end().parse::<u64>()?;

    Ok((rx_bytes, tx_bytes, Instant::now()))
}

/// Get network stats first sample
fn get_network_stats() -> anyhow::Result<NetworkPendingStats> {
    let mut stats: NetworkPendingStats = NetworkPendingStats::new();

    let mut dir_entries: Vec<DirEntry> = fs::read_dir("/sys/class/net")?
        .filter_map(Result::ok)
        .collect();
    dir_entries.sort_by_key(DirEntry::file_name);
    for dir_entry in dir_entries {
        let itf_name = dir_entry.file_name().clone().into_string().unwrap();
        if itf_name == "lo" {
            continue;
        }
        let itf_dir = dir_entry.path();

        let mut rx_bytes_file = File::open(itf_dir.join("statistics/rx_bytes"))?;
        let mut tx_bytes_file = File::open(itf_dir.join("statistics/tx_bytes"))?;
        let (rx_bytes, tx_bytes, ts) =
            read_interface_stats(&mut rx_bytes_file, &mut tx_bytes_file)?;

        rx_bytes_file.rewind()?;
        tx_bytes_file.rewind()?;

        let line_bps = if itf_dir.join("tun_flags").exists() {
            /* tun always report 10 Mbps even if we can exceed that limit */
            None
        } else {
            fs::read_to_string(itf_dir.join("speed"))
                .ok()
                .and_then(|speed_str| {
                    speed_str
                        .trim_end()
                        // Some interfaces (bridges) report -1
                        .parse::<u64>()
                        .map(|speed| speed * 1_000_000)
                        .ok()
                })
        };

        stats.insert(
            itf_name,
            PendingInterfaceStats {
                rx_bytes,
                tx_bytes,
                rx_bytes_file,
                tx_bytes_file,
                ts,
                line_bps,
            },
        );
    }

    Ok(stats)
}

/// Get network stats second sample and build interface stats
fn update_network_stats(pending_stats: &mut NetworkPendingStats) -> anyhow::Result<NetworkStats> {
    let mut stats = BTreeMap::new();

    for (itf_name, pending_itf_stats) in pending_stats.iter_mut() {
        // Ensure there is sufficient time between samples
        let now = Instant::now();
        let ms_since_first_sample = now.duration_since(pending_itf_stats.ts).as_millis() as u64;
        if ms_since_first_sample < MIN_DELAY_BETWEEN_NET_SAMPLES_MS {
            let sleep_delay_ms = MIN_DELAY_BETWEEN_NET_SAMPLES_MS - ms_since_first_sample;
            sleep(Duration::from_millis(sleep_delay_ms));
        }

        // Read sample
        let (rx_bytes2, tx_bytes2, ts2) = read_interface_stats(
            &mut pending_itf_stats.rx_bytes_file,
            &mut pending_itf_stats.tx_bytes_file,
        )?;

        // Convert to speed
        let ts_delta_ms = ts2.duration_since(pending_itf_stats.ts).as_millis();
        let rx_bps = 1000 * (rx_bytes2 - pending_itf_stats.rx_bytes) * 8 / ts_delta_ms as u64;
        let tx_bps = 1000 * (tx_bytes2 - pending_itf_stats.tx_bytes) * 8 / ts_delta_ms as u64;
        stats.insert(
            itf_name.to_string(),
            InterfaceStats {
                rx_bps,
                tx_bps,
                line_bps: pending_itf_stats.line_bps,
            },
        );
    }

    Ok(NetworkStats { interfaces: stats })
}

/// Colorize network speed string
fn colorize_speed(val: u64, line_rate: Option<u64>, s: String) -> String {
    if let Some(line_rate) = line_rate {
        if val >= line_rate * 90 / 100 {
            Red.paint(s).to_string()
        } else if val >= line_rate * 80 / 100 {
            Yellow.paint(s).to_string()
        } else {
            s
        }
    } else {
        s
    }
}

impl fmt::Display for NetworkStats {
    /// Output network stats
    #[expect(clippy::similar_names)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let unit = "b/s";
        let Some(max_itf_len) = self.interfaces.keys().map(String::len).max() else {
            return Ok(());
        };
        let mac_rx_str_len = self
            .interfaces
            .values()
            .map(|v| format_kmgt_si(v.rx_bps, unit).len())
            .max()
            .unwrap();
        let mac_tx_str_len = self
            .interfaces
            .values()
            .map(|v| format_kmgt_si(v.tx_bps, unit).len())
            .max()
            .unwrap();

        for (itf_name, itf_stats) in &self.interfaces {
            let name_pad = " ".repeat(max_itf_len - itf_name.len());
            let rx_str = format_kmgt_si(itf_stats.rx_bps, unit);
            let rx_pad = " ".repeat(mac_rx_str_len - rx_str.len());
            let tx_str = format_kmgt_si(itf_stats.tx_bps, unit);
            let tx_pad = " ".repeat(mac_tx_str_len - tx_str.len());
            writeln!(
                f,
                "{}:{} ↓ {}{}  ↑ {}{}",
                itf_name,
                name_pad,
                rx_pad,
                colorize_speed(itf_stats.rx_bps, itf_stats.line_bps, rx_str),
                tx_pad,
                colorize_speed(itf_stats.tx_bps, itf_stats.line_bps, tx_str)
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_network_stats() {
        let mut stats = BTreeMap::new();
        stats.insert(
            "i1".to_owned(),
            InterfaceStats {
                rx_bps: 1,
                tx_bps: 1_234_567,
                line_bps: None,
            },
        );
        stats.insert(
            "interface2".to_owned(),
            InterfaceStats {
                rx_bps: 1_234_567_890,
                tx_bps: 1_234,
                line_bps: None,
            },
        );
        stats.insert(
            "itf3".to_owned(),
            InterfaceStats {
                rx_bps: 799_999,
                tx_bps: 800_000,
                line_bps: Some(1_000_000),
            },
        );
        stats.insert(
            "itf4".to_owned(),
            InterfaceStats {
                rx_bps: 900_000,
                tx_bps: 899_999,
                line_bps: Some(1_000_000),
            },
        );
        stats.insert(
            "itf5".to_owned(),
            InterfaceStats {
                rx_bps: 900_000_001,
                tx_bps: 800_000_001,
                line_bps: Some(1_000_000_000),
            },
        );
        assert_eq!(
            format!("{}", NetworkStats { interfaces: stats }),
            "i1:         ↓      1 b/s  ↑   1.2 Mb/s\ninterface2: ↓   1.2 Gb/s  ↑   1.2 kb/s\nitf3:       ↓ 800.0 kb/s  ↑ \u{1b}[33m800.0 kb/s\u{1b}[0m\nitf4:       ↓ \u{1b}[31m900.0 kb/s\u{1b}[0m  ↑ \u{1b}[33m900.0 kb/s\u{1b}[0m\nitf5:       ↓ \u{1b}[31m900.0 Mb/s\u{1b}[0m  ↑ \u{1b}[33m800.0 Mb/s\u{1b}[0m\n"
        );
    }
}
