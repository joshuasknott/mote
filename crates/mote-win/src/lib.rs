//! mote-win: native Windows environment sensing.
//!
//! Each module wraps a slice of the Win32 API and converts it to plain data
//! for `mote-core`. Every sensor degrades gracefully: any API failure yields
//! a documented default, never a crash. Nothing here leaves the machine.

pub mod audio;
pub mod cursor;
pub mod events;
pub mod monitors;
pub mod stats;
pub mod taskbar;
pub mod windows;
pub mod world_build;

pub use audio::AudioMonitor;
pub use cursor::{idle_ms, poll_cursor, CursorSample, CursorTracker};
pub use events::WinEventListener;
pub use monitors::{query_monitors, virtual_screen_rect, MonitorInfo};
pub use stats::{cpu_usage_between, query_memory, CpuMeter, MemoryInfo};
pub use taskbar::{query_taskbar, TaskbarEdge, TaskbarInfo};
pub use windows::{enumerate_windows, foreground_is_fullscreen, TopLevelWindow};
pub use world_build::build_world;
