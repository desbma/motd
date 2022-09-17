//! Module common stuff

use std::fmt;
use std::sync::atomic::AtomicUsize;

use crate::fs::FsInfo;
use crate::load::LoadInfo;
use crate::mem::{MemInfo, SwapInfo};
use crate::net::NetworkStats;
use crate::systemd::FailedUnits;
use crate::temp::HardwareTemps;

pub enum ModuleData {
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
pub static CPU_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static TERM_COLUMNS: AtomicUsize = AtomicUsize::new(0);
