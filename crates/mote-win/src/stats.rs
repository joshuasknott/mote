//! CPU usage and memory pressure, sampled cheaply.
//!
//! CPU comes from `GetSystemTimes` deltas (kernel+user vs idle) — the same
//! primitive Task Manager uses — sampled at ~1 Hz. The pure ratio function
//! is unit-tested with synthetic ticks.

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

fn filetime_to_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CpuTimes {
    pub idle: u64,
    pub kernel: u64,
    pub user: u64,
}

impl CpuTimes {
    pub fn total(&self) -> u64 {
        self.kernel + self.user
    }
}

pub fn read_cpu_times() -> Option<CpuTimes> {
    unsafe {
        let mut idle = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).ok()?;
        Some(CpuTimes {
            idle: filetime_to_u64(&idle),
            kernel: filetime_to_u64(&kernel),
            user: filetime_to_u64(&user),
        })
    }
}

/// CPU busy fraction between two samples. 0 on degenerate input.
pub fn cpu_usage_between(prev: &CpuTimes, cur: &CpuTimes) -> f32 {
    let total = cur.total().saturating_sub(prev.total()) as f64;
    let idle = cur.idle.saturating_sub(prev.idle) as f64;
    if total <= 0.0 {
        return 0.0;
    }
    ((total - idle) / total).clamp(0.0, 1.0) as f32
}

/// 1 Hz CPU meter with smoothing. Feed by calling `sample()` once a second.
#[derive(Debug)]
pub struct CpuMeter {
    prev: Option<CpuTimes>,
    /// Smoothed 0..=1 usage.
    pub usage: f32,
}

impl CpuMeter {
    pub fn new() -> Self {
        Self {
            prev: None,
            usage: 0.0,
        }
    }

    pub fn sample(&mut self) {
        if let Some(cur) = read_cpu_times() {
            if let Some(prev) = self.prev {
                let u = cpu_usage_between(&prev, &cur);
                self.usage += (u - self.usage) * 0.5;
            }
            self.prev = Some(cur);
        }
    }
}

impl Default for CpuMeter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryInfo {
    /// Physical memory in use, 0..=1.
    pub load_01: f32,
    pub avail_mb: u64,
}

pub fn query_memory() -> MemoryInfo {
    unsafe {
        let mut st = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        match GlobalMemoryStatusEx(&mut st) {
            Ok(()) => MemoryInfo {
                load_01: (st.dwMemoryLoad as f32 / 100.0).clamp(0.0, 1.0),
                avail_mb: st.ullAvailPhys / (1024 * 1024),
            },
            Err(_) => MemoryInfo::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_ratio_math() {
        let a = CpuTimes {
            idle: 100,
            kernel: 200,
            user: 200,
        };
        let b = CpuTimes {
            idle: 150,
            kernel: 250,
            user: 250,
        };
        // total delta 100, idle delta 50 -> 50%.
        assert!((cpu_usage_between(&a, &b) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn cpu_degenerate_zero() {
        let a = CpuTimes::default();
        assert_eq!(cpu_usage_between(&a, &a), 0.0);
    }

    #[test]
    fn memory_query_sane() {
        let m = query_memory();
        assert!(m.load_01 > 0.0 && m.load_01 <= 1.0, "bad load {m:?}");
    }
}
