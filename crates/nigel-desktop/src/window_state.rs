//! Where the window was, so the next launch can put it back.
//!
//! Geometry is a convenience, so every failure here degrades to the default
//! window: an absent, corrupt, or unwritable state file costs the next launch
//! its position and nothing else. No dialog, no log — the next clean close
//! overwrites whatever was wrong.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The window's floor, shared with the builder in `main.rs`.
pub const MIN_WIDTH: f64 = 900.0;
/// See [`MIN_WIDTH`].
pub const MIN_HEIGHT: f64 = 700.0;

/// Window geometry in logical (scale-independent) units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
}

/// A monitor's usable region in logical units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The state file lives beside `settings.json`.
pub fn state_path() -> PathBuf {
    nigel_core::settings::config_dir().join("window-state.json")
}

/// A parsed state file, or `None` for any file that is absent or not the
/// expected shape.
pub fn load_from(path: &Path) -> Option<WindowGeometry> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Best-effort write; see the module contract.
pub fn save_to(path: &Path, geometry: WindowGeometry) {
    let Some(dir) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_string_pretty(&geometry) {
        let _ = std::fs::write(path, format!("{json}\n"));
    }
}

/// Saved geometry made safe to apply: floored at the window minimum, and
/// moved onto a visible monitor if the monitors changed since it was saved.
///
/// The host monitor is the one the saved window overlapped most, falling back
/// to the first. A monitor smaller than the minimum keeps the minimum — the
/// window overflows a tiny screen rather than shrink below what the layout
/// survives, which is the same promise `min_inner_size` makes.
pub fn clamp_restore(saved: WindowGeometry, monitors: &[MonitorArea]) -> WindowGeometry {
    let width = saved.width.max(MIN_WIDTH);
    let height = saved.height.max(MIN_HEIGHT);

    let Some(host) = host_monitor(saved, width, height, monitors) else {
        return WindowGeometry {
            width,
            height,
            ..saved
        };
    };

    let width = width.min(host.width).max(MIN_WIDTH);
    let height = height.min(host.height).max(MIN_HEIGHT);

    // min-then-max rather than clamp: a monitor narrower than the window
    // inverts the bounds, and the window then pins to the area's origin.
    let x = saved.x.min(host.x + host.width - width).max(host.x);
    let y = saved.y.min(host.y + host.height - height).max(host.y);

    WindowGeometry {
        width,
        height,
        x,
        y,
    }
}

fn host_monitor(
    saved: WindowGeometry,
    width: f64,
    height: f64,
    monitors: &[MonitorArea],
) -> Option<MonitorArea> {
    let overlap = |m: &MonitorArea| -> f64 {
        let w = (saved.x + width).min(m.x + m.width) - saved.x.max(m.x);
        let h = (saved.y + height).min(m.y + m.height) - saved.y.max(m.y);
        if w > 0.0 && h > 0.0 {
            w * h
        } else {
            0.0
        }
    };
    let best = monitors
        .iter()
        .copied()
        .max_by(|a, b| overlap(a).total_cmp(&overlap(b)))?;
    if overlap(&best) > 0.0 {
        Some(best)
    } else {
        monitors.first().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> MonitorArea {
        MonitorArea {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    #[test]
    fn a_state_file_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("window-state.json");
        let geometry = WindowGeometry {
            width: 1300.0,
            height: 900.0,
            x: 40.0,
            y: 60.0,
        };
        save_to(&path, geometry);
        assert_eq!(load_from(&path), Some(geometry));
    }

    #[test]
    fn an_absent_file_is_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(load_from(&dir.path().join("window-state.json")), None);
    }

    #[test]
    fn a_corrupt_file_is_none() {
        // Half a write, an edit by hand, another program's file — all the
        // same outcome: the default window, silently.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("window-state.json");
        std::fs::write(&path, "{\"width\": \"wide\"}").expect("write");
        assert_eq!(load_from(&path), None);
    }

    #[test]
    fn saving_into_an_unwritable_place_is_silent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("occupied");
        std::fs::write(&file, "").expect("write");
        // The parent "directory" is a file, so create_dir_all refuses.
        save_to(
            &file.join("window-state.json"),
            WindowGeometry {
                width: 1200.0,
                height: 820.0,
                x: 0.0,
                y: 0.0,
            },
        );
    }

    #[test]
    fn restore_floors_at_the_window_minimum() {
        let saved = WindowGeometry {
            width: 400.0,
            height: 300.0,
            x: 10.0,
            y: 10.0,
        };
        let restored = clamp_restore(saved, &[screen()]);
        assert_eq!((restored.width, restored.height), (MIN_WIDTH, MIN_HEIGHT));
    }

    #[test]
    fn restore_pulls_an_offscreen_window_back() {
        let saved = WindowGeometry {
            width: 1200.0,
            height: 820.0,
            x: 5000.0,
            y: -2000.0,
        };
        let restored = clamp_restore(saved, &[screen()]);
        assert_eq!((restored.x, restored.y), (1920.0 - 1200.0, 0.0));
    }

    #[test]
    fn restore_keeps_a_window_on_its_second_monitor() {
        let second = MonitorArea {
            x: 1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let saved = WindowGeometry {
            width: 1200.0,
            height: 820.0,
            x: 2000.0,
            y: 100.0,
        };
        let restored = clamp_restore(saved, &[screen(), second]);
        assert_eq!((restored.x, restored.y), (2000.0, 100.0));
    }

    #[test]
    fn restore_caps_to_a_smaller_monitor_but_never_below_minimum() {
        let laptop = MonitorArea {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 800.0,
        };
        let saved = WindowGeometry {
            width: 1800.0,
            height: 1000.0,
            x: 100.0,
            y: 100.0,
        };
        let restored = clamp_restore(saved, &[laptop]);
        assert_eq!((restored.width, restored.height), (1280.0, 800.0));

        let tiny = MonitorArea {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 480.0,
        };
        let restored = clamp_restore(saved, &[tiny]);
        assert_eq!((restored.width, restored.height), (MIN_WIDTH, MIN_HEIGHT));
        // Bounds invert on a screen narrower than the window; the window
        // pins to the area's origin instead of panicking in clamp().
        assert_eq!((restored.x, restored.y), (0.0, 0.0));
    }

    #[test]
    fn restore_without_monitor_information_only_floors() {
        let saved = WindowGeometry {
            width: 400.0,
            height: 1000.0,
            x: -50.0,
            y: 9000.0,
        };
        let restored = clamp_restore(saved, &[]);
        assert_eq!(
            restored,
            WindowGeometry {
                width: MIN_WIDTH,
                height: 1000.0,
                x: -50.0,
                y: 9000.0,
            }
        );
    }
}
