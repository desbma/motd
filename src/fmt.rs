/// Format numeric value with K/M/G prefix
#[allow(non_snake_case)]
pub fn format_kmg(val: u64, unit: &str) -> String {
    const G: u64 = 1024 * 1024 * 1024;
    const M: u64 = 1024 * 1024;
    const K: u64 = 1024;
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

/// Format numeric value with k/M/G prefix
#[allow(non_upper_case_globals)]
pub fn format_kmg_si(val: u64, unit: &str) -> String {
    const G_SI: u64 = 1_000_000_000;
    const M_SI: u64 = 1_000_000;
    const K_SI: u64 = 1_000;
    if val >= G_SI {
        format!("{:.2} G{}", val as f32 / G_SI as f32, unit)
    } else if val >= M_SI {
        format!("{:.2} M{}", val as f32 / M_SI as f32, unit)
    } else if val >= K_SI {
        format!("{:.2} k{}", val as f32 / K_SI as f32, unit)
    } else {
        format!("{} {}", val, unit)
    }
}
