//! Wayland `wlr-layer-shell` support for a Spotlight-style overlay window.
//!
//! # Background
//!
//! For a true "Spotlight-style" floating overlay we ideally want the
//! `wlr-layer-shell` protocol (on Wayland) or `override_redirect` (on X11).
//! Since `iced`/`winit` creates `xdg-shell` toplevel windows and does not
//! expose the underlying `wl_surface` for re-purposing, we cannot directly
//! create a layer-shell surface through the GUI framework.
//!
//! # What this module does
//!
//! - Detects whether the compositor is X11 or Wayland.
//! - If Wayland, attempts to open a lightweight `wayland-client` connection
//!   and check whether the compositor advertises `zwlr_layer_shell_v1`.
//! - Logs the result so developers / users can see whether the compositor
//!   supports layer-shell overlays.
//!
//! # Actual window behaviour
//!
//! The real "overlay" behaviour is achieved through `iced` window settings
//! configured in [`crate::run_app`]:
//!
//! | Setting | Effect |
//! |---|---|
//! | `decorations: false` | Borderless window |
//! | `transparent: true` | Alpha compositing for rounded corners |
//! | `level: AlwaysOnTop` | Above normal windows (Wayland + X11) |
//! | `override_redirect: true` | Bypass WM on X11 (true floating) |
//! | `position: SpecificWith(…)` | Top-centre placement (X11 only) |
//!
//! On Wayland, `AlwaysOnTop` is the closest approximation when the compositor
//! doesn't support layer-shell.  On compositors that *do* support it (e.g.
//! sway, Hyprland), the user can add a window rule to force the launcher into
//! the overlay layer (see module-level docs).

use std::sync::OnceLock;

/// Whether we successfully detected a Wayland session.
static IS_WAYLAND: OnceLock<bool> = OnceLock::new();

/// Returns `true` if we detected a Wayland display server.
#[allow(dead_code)]
pub fn is_wayland() -> bool {
    IS_WAYLAND.get().copied().unwrap_or(false)
}

/// Returns `true` if the compositor advertises `zwlr_layer_shell_v1`.
#[allow(dead_code)]
pub fn has_layer_shell() -> bool {
    is_wayland()
        && WAYLAND_HAS_LAYER_SHELL
            .get()
            .copied()
            .unwrap_or(false)
}

static WAYLAND_HAS_LAYER_SHELL: OnceLock<bool> = OnceLock::new();

/// Perform initialisation: detect display server and log diagnostics.
///
/// Call this once before creating the iced window.  It never blocks
/// for more than a single Wayland round-trip.
pub fn init() {
    if cfg!(not(target_os = "linux")) {
        tracing::debug!("layer_shell: not on Linux – skipping");
        return;
    }

    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(false);

    let _ = IS_WAYLAND.set(is_wayland);

    if !is_wayland {
        tracing::info!(
            "window: X11 detected – using override_redirect + always-on-top"
        );
        return;
    }

    // On Wayland, try to query the compositor for layer-shell support.
    match probe_layer_shell() {
        Ok(true) => {
            tracing::info!(
                "window: Wayland compositor supports wlr-layer-shell ✓"
            );
            tracing::info!(
                "window: for a true overlay, add a window rule, e.g. for sway:\n  \
                 for_window [app_id=\"obsidian-launcher\" shell=\"layer_shell\"] \\\n  \
                   move position 0 0, resize set 700 400"
            );
            let _ = WAYLAND_HAS_LAYER_SHELL.set(true);
        }
        Ok(false) => {
            tracing::info!(
                "window: Wayland compositor does NOT advertise wlr-layer-shell;\n  \
                 falling back to xdg-shell always-on-top"
            );
        }
        Err(e) => {
            tracing::warn!(
                "window: failed to probe wlr-layer-shell ({}) – using xdg-shell",
                e
            );
        }
    }
}

/// Open a minimal Wayland connection and check for the layer-shell global.
fn probe_layer_shell() -> Result<bool, Box<dyn std::error::Error>> {
    use wayland_client::{Connection, Dispatch, QueueHandle};

    struct ProbeState {
        found_layer_shell: bool,
    }

    impl Dispatch<wayland_client::protocol::wl_registry::WlRegistry, ()> for ProbeState {
        fn event(
            state: &mut Self,
            _: &wayland_client::protocol::wl_registry::WlRegistry,
            event: wayland_client::protocol::wl_registry::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wayland_client::protocol::wl_registry::Event::Global {
                interface, ..
            } = event
            {
                if &interface == "zwlr_layer_shell_v1" {
                    state.found_layer_shell = true;
                }
            }
        }
    }

    let conn = Connection::connect_to_env()?;
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    conn.display().get_registry(&qh, ());

    let mut state = ProbeState { found_layer_shell: false };
    event_queue.roundtrip(&mut state)?;

    Ok(state.found_layer_shell)
}
