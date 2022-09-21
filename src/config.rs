//! Local configuration

/// Local configuration
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    /// Filesystem module config
    pub fs: FsConfig,

    /// Temp module config
    pub temp: TempConfig,
}

/// Filesystem module config
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct FsConfig {
    /// Exclude filesystem whose type match any of theses regexs
    #[serde(with = "serde_regex")]
    pub mount_type_blacklist: Vec<regex::Regex>,
    /// Exclude filesystem whose mount point match any of theses regexs
    #[serde(with = "serde_regex")]
    pub mount_path_blacklist: Vec<regex::Regex>,
}

/// Temp module config
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct TempConfig {
    /// Exclude temp probes label (/sys/class/hwmon/hwmon*/temp*_label files) matching any of theses regexs
    #[serde(with = "serde_regex")]
    pub hwmon_label_blacklist: Vec<regex::Regex>,
}

/// Parse local configuration
pub fn parse_config() -> anyhow::Result<Config> {
    let binary_name = env!("CARGO_PKG_NAME");
    let xdg_dirs = xdg::BaseDirectories::with_prefix(binary_name)?;
    let config = if let Some(config_filepath) = xdg_dirs.find_config_file("config.toml") {
        let toml_data = std::fs::read_to_string(config_filepath)?;
        toml::from_str(&toml_data)?
    } else {
        Config::default()
    };
    Ok(config)
}
