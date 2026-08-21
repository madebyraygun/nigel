//! Where the window was, so the next launch can put it back.
//!
//! Geometry is a convenience, so every failure here degrades to the default
//! window: an absent, corrupt, or unwritable state file costs the next launch
//! its position and nothing else. No dialog, no log — the next clean close
//! overwrites whatever was wrong.
//!
//! The file keeps the last *normal* frame — never a minimized or fullscreen
//! reading, and a maximized window sets a flag instead of overwriting the
//! frame it will unmaximize back to.
//!
//! Coordinate model: `width`/`height` are the content (inner) size and
//! `x`/`y` the frame (outer) top-left, both in logical units, with
//! `extra_width`/`extra_height` carrying the decoration difference between
//! the two rectangles and `scale` the monitor scale at save time. Restore
//! math runs on the full frame rectangle in whichever coordinate space is
//! globally coherent for the platform — logical points on macOS, physical
//! pixels elsewhere — because the other space does not form one coordinate
//! system across mixed-DPI monitors.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The window's floor, shared with the builder in `main.rs`.
pub const MIN_WIDTH: f64 = 900.0;
/// See [`MIN_WIDTH`].
pub const MIN_HEIGHT: f64 = 700.0;
/// A fresh window's content size, shared with the builder in `main.rs`.
pub const DEFAULT_WIDTH: f64 = 1200.0;
/// See [`DEFAULT_WIDTH`].
pub const DEFAULT_HEIGHT: f64 = 820.0;

/// How long a window has to hold still before its geometry is written.
const SETTLE: Duration = Duration::from_millis(300);

fn default_scale() -> f64 {
    1.0
}

/// The saved window state, in logical (scale-independent) units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Content (inner) size.
    pub width: f64,
    pub height: f64,
    /// Frame (outer) top-left.
    pub x: f64,
    pub y: f64,
    /// Frame size minus content size — the decorations.
    #[serde(default)]
    pub extra_width: f64,
    #[serde(default)]
    pub extra_height: f64,
    /// The window's monitor scale when saved.
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default)]
    pub maximized: bool,
}

/// An axis-aligned rectangle; monitors and window frames share the shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A monitor's usable region.
pub type MonitorArea = Rect;

/// The state file lives beside `settings.json`.
pub fn state_path() -> PathBuf {
    nigel_core::settings::config_dir().join("window-state.json")
}

/// A parsed state file, or `None` for any file that is absent or not the
/// expected shape.
pub fn load_from(path: &Path) -> Option<WindowGeometry> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut geometry: WindowGeometry = serde_json::from_str(&content).ok()?;
    if !(geometry.scale.is_finite() && geometry.scale > 0.0) {
        geometry.scale = 1.0;
    }
    Some(geometry)
}

/// Best-effort write; see the module contract. The config directory holds
/// `settings.json` too, so it gets the same permission posture whichever
/// file creates it first.
pub fn save_to(path: &Path, geometry: WindowGeometry) {
    let Some(dir) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let _ = nigel_core::settings::restrict_dir_permissions(dir);
    if let Ok(json) = serde_json::to_string_pretty(&geometry) {
        if std::fs::write(path, format!("{json}\n")).is_ok() {
            let _ = nigel_core::settings::restrict_file_permissions(path);
        }
    }
}

/// What the builder and the freshly built window should be told.
///
/// `inner_width`/`inner_height` are logical, for the builder. `frame_x`/
/// `frame_y` are in the caller's clamp space — the same space its monitors
/// were in — for `set_position` after the build.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RestorePlan {
    pub inner_width: f64,
    pub inner_height: f64,
    pub frame_x: f64,
    pub frame_y: f64,
    pub maximized: bool,
}

/// Saved geometry made safe to apply: the frame rectangle — decorations
/// included, so a clamped window's real bottom edge respects the work area —
/// floored at the window minimum and moved onto a visible monitor if the
/// monitors changed since it was saved.
///
/// `space_scale` converts the saved logical units into the space `monitors`
/// are in: `1.0` where monitors are logical (macOS), the saved scale where
/// they are physical (Windows, Linux). Callers with no monitor information
/// skip positioning instead of calling this.
pub fn plan_restore(saved: &WindowGeometry, monitors: &[Rect], space_scale: f64) -> RestorePlan {
    let k = space_scale;
    let frame = Rect {
        x: saved.x * k,
        y: saved.y * k,
        width: (saved.width + saved.extra_width) * k,
        height: (saved.height + saved.extra_height) * k,
    };
    let clamped = clamp_frame(
        frame,
        (MIN_WIDTH + saved.extra_width) * k,
        (MIN_HEIGHT + saved.extra_height) * k,
        monitors,
    );
    RestorePlan {
        inner_width: clamped.width / k - saved.extra_width,
        inner_height: clamped.height / k - saved.extra_height,
        frame_x: clamped.x,
        frame_y: clamped.y,
        maximized: saved.maximized,
    }
}

/// The frame-rectangle clamp under [`plan_restore`]: floored at `min_*`,
/// then confined to the host monitor.
///
/// The host monitor is the one the frame overlapped most, falling back to
/// the first. A monitor smaller than the minimum keeps the minimum — the
/// window overflows a tiny screen rather than shrink below what the layout
/// survives, which is the same promise `min_inner_size` makes.
pub fn clamp_frame(saved: Rect, min_width: f64, min_height: f64, monitors: &[Rect]) -> Rect {
    let width = saved.width.max(min_width);
    let height = saved.height.max(min_height);

    let Some(host) = host_monitor(
        Rect {
            width,
            height,
            ..saved
        },
        monitors,
    ) else {
        return Rect {
            width,
            height,
            ..saved
        };
    };

    let width = width.min(host.width).max(min_width);
    let height = height.min(host.height).max(min_height);

    // min-then-max rather than clamp: a monitor narrower than the window
    // inverts the bounds, and the window then pins to the area's origin.
    let x = saved.x.min(host.x + host.width - width).max(host.x);
    let y = saved.y.min(host.y + host.height - height).max(host.y);

    Rect {
        width,
        height,
        x,
        y,
    }
}

fn host_monitor(frame: Rect, monitors: &[Rect]) -> Option<Rect> {
    let overlap = |m: &Rect| -> f64 {
        let w = (frame.x + frame.width).min(m.x + m.width) - frame.x.max(m.x);
        let h = (frame.y + frame.height).min(m.y + m.height) - frame.y.max(m.y);
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

/// Writes window observations to the state file: frames debounce until the
/// window holds still, [`GeometrySaver::save_now`] flushes on close.
///
/// Observations arrive from the event thread; the write happens on a
/// background thread that coalesces a drag's stream of events into one
/// write. Quit cannot outrun the settle window: the shell flushes with
/// [`GeometrySaver::save_now`] at loop teardown, which every quit path
/// reaches.
pub struct GeometrySaver {
    path: PathBuf,
    last: Arc<Mutex<Option<WindowGeometry>>>,
    nudge: Sender<()>,
}

impl GeometrySaver {
    /// A saver for `path`, seeded with the file's current state so a
    /// maximized-only session still knows the frame to fall back to.
    pub fn spawn(path: PathBuf) -> Self {
        let last = Arc::new(Mutex::new(load_from(&path)));
        let (nudge, nudged) = std::sync::mpsc::channel();
        let write_path = path.clone();
        let write_last = Arc::clone(&last);
        std::thread::spawn(move || write_after_settle(nudged, &write_path, &write_last));
        Self { path, last, nudge }
    }

    /// A normal (not minimized, maximized, or fullscreen) frame reading.
    pub fn observe_frame(&self, geometry: WindowGeometry) {
        *self.last.lock().expect("geometry lock") = Some(geometry);
        let _ = self.nudge.send(());
    }

    /// The window is maximized: keep the frame it will unmaximize back to,
    /// remember only the flag. Without any known frame there is nothing
    /// worth writing.
    pub fn observe_maximized(&self) {
        if let Some(geometry) = self.last.lock().expect("geometry lock").as_mut() {
            geometry.maximized = true;
        }
        let _ = self.nudge.send(());
    }

    /// Write the current state synchronously — the close path, where the
    /// process may not outlive the settle window.
    pub fn save_now(&self) {
        if let Some(geometry) = *self.last.lock().expect("geometry lock") {
            save_to(&self.path, geometry);
        }
    }
}

fn write_after_settle(nudged: Receiver<()>, path: &Path, last: &Mutex<Option<WindowGeometry>>) {
    while nudged.recv().is_ok() {
        while nudged.recv_timeout(SETTLE).is_ok() {}
        if let Some(geometry) = *last.lock().expect("geometry lock") {
            save_to(path, geometry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    fn saved(width: f64, height: f64, x: f64, y: f64) -> WindowGeometry {
        WindowGeometry {
            width,
            height,
            x,
            y,
            extra_width: 0.0,
            extra_height: 0.0,
            scale: 1.0,
            maximized: false,
        }
    }

    #[test]
    fn a_state_file_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("window-state.json");
        let geometry = WindowGeometry {
            extra_height: 28.0,
            scale: 2.0,
            maximized: true,
            ..saved(1300.0, 900.0, 40.0, 60.0)
        };
        save_to(&path, geometry);
        assert_eq!(load_from(&path), Some(geometry));
    }

    #[test]
    fn a_pre_decoration_state_file_still_loads() {
        // The fields the file did not always have default rather than
        // invalidate an existing installation's state.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("window-state.json");
        std::fs::write(
            &path,
            "{\"width\": 1300.0, \"height\": 900.0, \"x\": 40.0, \"y\": 60.0}",
        )
        .expect("write");
        assert_eq!(load_from(&path), Some(saved(1300.0, 900.0, 40.0, 60.0)));
    }

    #[test]
    fn a_nonsense_scale_falls_back_to_one() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("window-state.json");
        std::fs::write(
            &path,
            "{\"width\": 1300.0, \"height\": 900.0, \"x\": 40.0, \"y\": 60.0, \"scale\": 0.0}",
        )
        .expect("write");
        assert_eq!(load_from(&path).map(|g| g.scale), Some(1.0));
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
            saved(1200.0, 820.0, 0.0, 0.0),
        );
    }

    #[cfg(unix)]
    #[test]
    fn saving_restricts_the_config_directory() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("nigel");
        save_to(
            &config.join("window-state.json"),
            saved(1200.0, 820.0, 0.0, 0.0),
        );
        let mode = std::fs::metadata(&config)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "the config dir is not private");
    }

    #[test]
    fn restore_floors_at_the_window_minimum() {
        let plan = plan_restore(&saved(400.0, 300.0, 10.0, 10.0), &[screen()], 1.0);
        assert_eq!(
            (plan.inner_width, plan.inner_height),
            (MIN_WIDTH, MIN_HEIGHT)
        );
    }

    #[test]
    fn restore_pulls_an_offscreen_window_back() {
        let plan = plan_restore(&saved(1200.0, 820.0, 5000.0, -2000.0), &[screen()], 1.0);
        assert_eq!((plan.frame_x, plan.frame_y), (1920.0 - 1200.0, 0.0));
    }

    #[test]
    fn restore_keeps_a_window_on_its_second_monitor() {
        let second = Rect {
            x: 1920.0,
            ..screen()
        };
        let plan = plan_restore(
            &saved(1200.0, 820.0, 2000.0, 100.0),
            &[screen(), second],
            1.0,
        );
        assert_eq!((plan.frame_x, plan.frame_y), (2000.0, 100.0));
    }

    #[test]
    fn restore_counts_decorations_against_the_work_area() {
        // A frame is a title bar taller than its content: a content-rect
        // clamp would let the real bottom edge overhang the work area by
        // that height.
        let geometry = WindowGeometry {
            extra_height: 28.0,
            ..saved(1200.0, 820.0, 100.0, 500.0)
        };
        let plan = plan_restore(&geometry, &[screen()], 1.0);
        assert_eq!(plan.frame_y + 820.0 + 28.0, 1080.0);
        assert_eq!(plan.inner_height, 820.0);
    }

    #[test]
    fn restore_clamps_in_physical_space_with_the_saved_scale() {
        // Windows-shaped mixed DPI: primary 1920@100%, secondary 4K@200% at
        // physical x=1920. Their per-monitor logical rects would overlap;
        // their physical rects do not. A window saved on the secondary at
        // physical x=3000 (logical 1500 at scale 2) must stay there.
        let primary = screen();
        let secondary = Rect {
            x: 1920.0,
            y: 0.0,
            width: 3840.0,
            height: 2160.0,
        };
        let geometry = WindowGeometry {
            scale: 2.0,
            ..saved(1200.0, 820.0, 1500.0, 50.0)
        };
        let plan = plan_restore(&geometry, &[primary, secondary], geometry.scale);
        assert_eq!((plan.frame_x, plan.frame_y), (3000.0, 100.0));
        // The builder's inner size stays logical.
        assert_eq!((plan.inner_width, plan.inner_height), (1200.0, 820.0));
    }

    #[test]
    fn restore_carries_the_maximized_flag() {
        let geometry = WindowGeometry {
            maximized: true,
            ..saved(1200.0, 820.0, 100.0, 100.0)
        };
        assert!(plan_restore(&geometry, &[screen()], 1.0).maximized);
    }

    #[test]
    fn restore_caps_to_a_smaller_monitor_but_never_below_minimum() {
        let laptop = Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 800.0,
        };
        let big = saved(1800.0, 1000.0, 100.0, 100.0);
        let plan = plan_restore(&big, &[laptop], 1.0);
        assert_eq!((plan.inner_width, plan.inner_height), (1280.0, 800.0));

        let tiny = Rect {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 480.0,
        };
        let plan = plan_restore(&big, &[tiny], 1.0);
        assert_eq!(
            (plan.inner_width, plan.inner_height),
            (MIN_WIDTH, MIN_HEIGHT)
        );
        // Bounds invert on a screen narrower than the window; the window
        // pins to the area's origin instead of panicking in clamp().
        assert_eq!((plan.frame_x, plan.frame_y), (0.0, 0.0));
    }

    #[test]
    fn a_saver_coalesces_a_drag_into_the_last_frame() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("window-state.json");
        let saver = GeometrySaver::spawn(path.clone());
        for x in [10.0, 20.0, 30.0] {
            saver.observe_frame(saved(1200.0, 820.0, x, 0.0));
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(geometry) = load_from(&path) {
                assert_eq!(geometry.x, 30.0);
                break;
            }
            assert!(std::time::Instant::now() < deadline, "saver never wrote");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_close_writes_without_waiting() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("window-state.json");
        let saver = GeometrySaver::spawn(path.clone());
        saver.observe_frame(saved(1200.0, 820.0, 40.0, 60.0));
        saver.save_now();
        assert_eq!(load_from(&path), Some(saved(1200.0, 820.0, 40.0, 60.0)));
    }

    #[test]
    fn maximizing_keeps_the_normal_frame_and_sets_the_flag() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("window-state.json");
        let saver = GeometrySaver::spawn(path.clone());
        saver.observe_frame(saved(1200.0, 820.0, 40.0, 60.0));
        saver.observe_maximized();
        saver.save_now();
        let written = load_from(&path).expect("state written");
        assert!(written.maximized);
        assert_eq!((written.width, written.x), (1200.0, 40.0));
    }

    #[test]
    fn a_maximized_only_session_with_no_known_frame_writes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("window-state.json");
        let saver = GeometrySaver::spawn(path.clone());
        saver.observe_maximized();
        saver.save_now();
        assert_eq!(load_from(&path), None);
    }
}
