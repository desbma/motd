/// Format numeric value with K/M/G/T prefix
pub fn format_kmgt(val: u64, unit: &str) -> String {
    const K: u64 = 1024;
    const M: u64 = K * 1024;
    const G: u64 = M * 1024;
    const T: u64 = G * 1024;
    if val >= T {
        format!("{:.1} T{}", val as f32 / T as f32, unit)
    } else if val >= G {
        format!("{:.1} G{}", val as f32 / G as f32, unit)
    } else if val >= M {
        format!("{:.1} M{}", val as f32 / M as f32, unit)
    } else if val >= K {
        format!("{:.1} K{}", val as f32 / K as f32, unit)
    } else {
        format!("{} {}", val, unit)
    }
}

/// Format numeric value with k/M/G/T prefix
pub fn format_kmgt_si(val: u64, unit: &str) -> String {
    const K_SI: u64 = 1000;
    const M_SI: u64 = K_SI * 1000;
    const G_SI: u64 = M_SI * 1000;
    const T_SI: u64 = G_SI * 1000;
    if val >= T_SI {
        format!("{:.1} T{}", val as f32 / T_SI as f32, unit)
    } else if val >= G_SI {
        format!("{:.1} G{}", val as f32 / G_SI as f32, unit)
    } else if val >= M_SI {
        format!("{:.1} M{}", val as f32 / M_SI as f32, unit)
    } else if val >= K_SI {
        format!("{:.1} k{}", val as f32 / K_SI as f32, unit)
    } else {
        format!("{} {}", val, unit)
    }
}
