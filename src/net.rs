use std::collections::BTreeMap;
use std::error;
use std::fs;
use std::fs::{DirEntry, File};
use std::io::{Read, Seek, SeekFrom};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// Network interface pending stats
pub struct InterfacePendingStats {
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
}

pub type NetworkPendingStats = BTreeMap<String, InterfacePendingStats>;

/// Network interface stats
pub struct InterfaceStats {
    /// Rx bits/s
    rx_bps: u64,
    /// Tx bits/s
    tx_bps: u64,
}

pub type NetworkStats = BTreeMap<String, InterfaceStats>;

const MIN_DELAY_BETWEEN_NET_SAMPLES_MS: u64 = 30;

fn read_interface_stats(
    rx_bytes_file: &mut File,
    tx_bytes_file: &mut File,
) -> Result<(u64, u64, Instant), Box<dyn error::Error>> {
    let mut rx_str = String::new();
    rx_bytes_file.read_to_string(&mut rx_str)?;
    let rx_bytes = rx_str.trim_end().parse::<u64>()?;

    let mut tx_str = String::new();
    tx_bytes_file.read_to_string(&mut tx_str)?;
    let tx_bytes = tx_str.trim_end().parse::<u64>()?;

    Ok((rx_bytes, tx_bytes, Instant::now()))
}

/// Get network stats sample
pub fn get_network_stats() -> Result<NetworkPendingStats, Box<dyn error::Error>> {
    let mut stats: NetworkPendingStats = NetworkPendingStats::new();

    let mut dir_entries: Vec<DirEntry> = fs::read_dir("/sys/class/net")?
        .filter_map(Result::ok)
        .collect();
    dir_entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    for dir_entry in dir_entries {
        let itf_name = dir_entry.file_name().to_os_string().into_string().unwrap();
        if itf_name == "lo" {
            continue;
        }
        let itf_dir = dir_entry.path().into_os_string().into_string().unwrap();

        let mut rx_bytes_file = File::open(format!("{}/statistics/rx_bytes", itf_dir))?;
        let mut tx_bytes_file = File::open(format!("{}/statistics/tx_bytes", itf_dir))?;
        let (rx_bytes, tx_bytes, ts) =
            read_interface_stats(&mut rx_bytes_file, &mut tx_bytes_file)?;

        rx_bytes_file.seek(SeekFrom::Start(0))?;
        tx_bytes_file.seek(SeekFrom::Start(0))?;

        stats.insert(
            itf_name,
            InterfacePendingStats {
                rx_bytes,
                tx_bytes,
                rx_bytes_file,
                tx_bytes_file,
                ts,
            },
        );
    }

    Ok(stats)
}

/// Get network stats second sample and build interface stats
pub fn update_network_stats(
    pending_stats: &mut NetworkPendingStats,
) -> Result<NetworkStats, Box<dyn error::Error>> {
    let mut stats: NetworkStats = NetworkStats::new();

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
        stats.insert(itf_name.to_string(), InterfaceStats { rx_bps, tx_bps });
    }

    Ok(stats)
}

fn format_kmg(val: u64, unit: &str) -> String {
    const G: u64 = 1_000_000_000;
    const M: u64 = 1_000_000;
    const K: u64 = 1_000;
    if val >= G {
        format!("{:.2} G{}", val as f32 / G as f32, unit)
    } else if val >= M {
        format!("{:.2} M{}", val as f32 / M as f32, unit)
    } else if val >= K {
        format!("{:.2} K{}", val as f32 / K as f32, unit)
    } else {
        format!("{} {}", val, unit)
    }
}

/// Output network stats
pub fn output_network_stats(stats: NetworkStats) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    let unit = "b/s";
    let max_itf_len = match (&stats).iter().map(|(k, _v)| k.len()).max() {
        Some(m) => m,
        None => return lines,
    };
    let mac_rx_str_len = (&stats)
        .iter()
        .map(|(_k, v)| format_kmg(v.rx_bps, unit).len())
        .max()
        .unwrap();
    let mac_tx_str_len = (&stats)
        .iter()
        .map(|(_k, v)| format_kmg(v.tx_bps, unit).len())
        .max()
        .unwrap();

    for (itf_name, itf_stats) in stats {
        let name_pad = " ".repeat(max_itf_len - itf_name.len());
        let rx_str = format_kmg(itf_stats.rx_bps, unit);
        let rx_pad = " ".repeat(mac_rx_str_len - rx_str.len());
        let tx_str = format_kmg(itf_stats.tx_bps, unit);
        let tx_pad = " ".repeat(mac_tx_str_len - tx_str.len());
        let line = format!(
            "{}:{} ↓ {}{}  ↑ {}{}",
            itf_name,
            name_pad,
            rx_pad,
            format_kmg(itf_stats.rx_bps, unit),
            tx_pad,
            format_kmg(itf_stats.tx_bps, unit)
        );
        lines.push(line);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_network_stats() {
        let mut stats = NetworkStats::new();
        stats.insert(
            "i1".to_string(),
            InterfaceStats {
                rx_bps: 1,
                tx_bps: 1_234_567,
            },
        );
        stats.insert(
            "interface2".to_string(),
            InterfaceStats {
                rx_bps: 1_234_567_890,
                tx_bps: 1_234,
            },
        );
        assert_eq!(
            output_network_stats(stats),
            [
                "i1:         ↓     1 b/s  ↑ 1.23 Mb/s",
                "interface2: ↓ 1.23 Gb/s  ↑ 1.23 Kb/s"
            ]
        );
    }
}
