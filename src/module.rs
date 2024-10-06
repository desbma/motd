//! Module common stuff

use std::{fmt, sync::atomic::AtomicUsize};

use crate::{
    fs::FsInfo,
    load::LoadInfo,
    mem::{MemInfo, SwapInfo},
    net::NetworkStats,
    systemd::FailedUnits,
    temp::HardwareTemps,
};

pub(crate) enum ModuleData {
    Load(LoadInfo),
    Memory(MemInfo),
    Swap(SwapInfo),
    Fs(FsInfo),
    HardwareTemps(HardwareTemps),
    Systemd(FailedUnits),
    Network(NetworkStats),
}

// TODO use enum dispatch
impl fmt::Display for ModuleData {
    /// Output load information
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Load(i) => i.fmt(f),
            Self::Memory(i) => i.fmt(f),
            Self::Swap(i) => i.fmt(f),
            Self::Fs(i) => i.fmt(f),
            Self::HardwareTemps(i) => i.fmt(f),
            Self::Systemd(i) => i.fmt(f),
            Self::Network(i) => i.fmt(f),
        }
    }
}

// Global stuff, intitialized by main function or unit tests
pub(crate) static CPU_COUNT: AtomicUsize = AtomicUsize::new(0);
pub(crate) static TERM_COLUMNS: AtomicUsize = AtomicUsize::new(0);
