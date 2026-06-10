use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use evdev::{Device, EventType, InputEventKind, Key};
use tracing::{info, warn};

use crate::config::Config;

pub struct HotkeyDaemon {
    config: Config,
    running: bool,
}

// Linux ENODEV = 19 (No such device)
const ENODEV: i32 = 19;

impl HotkeyDaemon {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            running: true,
        }
    }

    pub fn run(&mut self) {
        info!("Hotkey daemon starting");
        info!("Listening for hotkey: {:?}", self.config.hotkey);

        let hotkey = self.config.hotkey.clone().unwrap_or_default();
        let (target_modifiers, target_key) = parse_hotkey(&hotkey);
        info!("Parsed hotkey: modifiers={:?}, key={:?}", target_modifiers, target_key);

        let pressed_modifiers: Arc<Mutex<HashSet<Key>>> = Arc::new(Mutex::new(HashSet::new()));
        let active_paths: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

        // Initial scan & spawn
        self.scan_and_spawn(&active_paths, &pressed_modifiers, &target_modifiers, target_key);

        // Main loop: rescan periodically and clean up finished threads
        let mut spawn_count: usize = 0;
        while self.running {
            thread::sleep(Duration::from_secs(2));

            // Remove paths whose threads have exited (disconnected keyboards)
            {
                let mut active = active_paths.lock().unwrap();
                active.retain(|p| {
                    // Keep only paths whose device file still exists
                    p.exists()
                });
            }

            self.scan_and_spawn(&active_paths, &pressed_modifiers, &target_modifiers, target_key);
            spawn_count += 1;

            if spawn_count % 30 == 0 {
                info!("Hotkey daemon still running, {} active path(s)",
                    active_paths.lock().unwrap().len());
            }
        }
    }

    fn scan_and_spawn(
        &self,
        active_paths: &Arc<Mutex<HashSet<PathBuf>>>,
        pressed_modifiers: &Arc<Mutex<HashSet<Key>>>,
        target_modifiers: &[Key],
        target_key: Key,
    ) {
        let devices = self.find_keyboard_devices_with_paths();
        if devices.is_empty() {
            return;
        }

        for (path, mut device) in devices {
            let path_str = path.to_string_lossy().to_string();
            {
                let active = active_paths.lock().unwrap();
                if active.contains(&path) {
                    continue; // Already monitored
                }
            }

            // Register this path and spawn a monitoring thread
            active_paths.lock().unwrap().insert(path.clone());

            let mods = Arc::clone(pressed_modifiers);
            let target_mods = target_modifiers.to_vec();
            let target_k = target_key;
            let config = self.config.clone();
            let active_paths_clone = Arc::clone(active_paths);
            let dev_path = path.clone();

            thread::spawn(move || {
                let dev_name = device.name().unwrap_or("unknown").to_string();
                info!("Listening on keyboard: {} ({})", dev_name, path_str);

                'event_loop: loop {
                    let events = match device.fetch_events() {
                        Ok(events) => events,
                        Err(e) => {
                            let is_disconnected = e.raw_os_error() == Some(ENODEV);
                            if is_disconnected {
                                info!("Keyboard disconnected: {} ({})", dev_name, path_str);
                            } else {
                                warn!("Failed to fetch events from {}: {}", dev_name, e);
                                thread::sleep(Duration::from_millis(100));
                                continue;
                            }
                            break 'event_loop;
                        }
                    };

                    for event in events {
                        let event_type = event.event_type();
                        if event_type != EventType::KEY {
                            continue;
                        }

                        let key = match event.kind() {
                            InputEventKind::Key(k) => k,
                            _ => continue,
                        };

                        let is_press = event.value() == 1;
                        let is_release = event.value() == 0;

                        if is_press {
                            if is_modifier(key) {
                                if let Ok(mut map) = mods.lock() {
                                    map.insert(key);
                                }
                            }

                            if key == target_k {
                                let map = match mods.lock() {
                                    Ok(m) => m,
                                    Err(_) => {
                                        warn!("Mutex poisoned, skipping key event");
                                        continue;
                                    }
                                };
                                let pressed_count = modifier_group_count(&map);
                                let target_count = target_modifier_group_count(&target_mods);
                                let mods_match = target_mods.iter().all(|m| modifier_active(*m, &map));

                                let hotkey_fires = if target_count > 0 {
                                    mods_match && pressed_count == target_count
                                } else {
                                    pressed_count == 0
                                };

                                if hotkey_fires {
                                    info!("Hotkey detected! Launching app...");
                                    launch_app(&config);
                                }
                            }
                        } else if is_release {
                            if let Ok(mut map) = mods.lock() {
                                map.remove(&key);
                            }
                        }
                    }
                }

                // On exit (disconnect), remove from active paths so reconnection is possible
                active_paths_clone.lock().unwrap().remove(&dev_path);
            });
        }
    }

    fn find_keyboard_devices_with_paths(&self) -> Vec<(PathBuf, Device)> {
        let mut devices = Vec::new();

        let input_path = Path::new("/dev/input");
        if !input_path.exists() {
            warn!("/dev/input does not exist");
            return devices;
        }

        let entries = match fs::read_dir(input_path) {
            Ok(entries) => entries,
            Err(e) => {
                warn!("Cannot read /dev/input: {}", e);
                return devices;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            let filename = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => continue,
            };

            if !filename.starts_with("event") {
                continue;
            }

            match Device::open(&path) {
                Ok(device) => {
                    if device.supported_keys().map_or(false, |keys| keys.contains(Key::KEY_A)) {
                        devices.push((path, device));
                    }
                }
                Err(e) => {
                    warn!("Cannot open {}: {}", path.display(), e);
                }
            }
        }

        devices
    }
}

fn is_modifier(key: Key) -> bool {
    matches!(
        key,
        Key::KEY_LEFTCTRL
            | Key::KEY_RIGHTCTRL
            | Key::KEY_LEFTALT
            | Key::KEY_RIGHTALT
            | Key::KEY_LEFTSHIFT
            | Key::KEY_RIGHTSHIFT
            | Key::KEY_LEFTMETA
            | Key::KEY_RIGHTMETA
    )
}

fn modifier_active(key: Key, pressed: &HashSet<Key>) -> bool {
    match key {
        Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => {
            pressed.contains(&Key::KEY_LEFTCTRL) || pressed.contains(&Key::KEY_RIGHTCTRL)
        }
        Key::KEY_LEFTALT | Key::KEY_RIGHTALT => {
            pressed.contains(&Key::KEY_LEFTALT) || pressed.contains(&Key::KEY_RIGHTALT)
        }
        Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => {
            pressed.contains(&Key::KEY_LEFTSHIFT) || pressed.contains(&Key::KEY_RIGHTSHIFT)
        }
        Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => {
            pressed.contains(&Key::KEY_LEFTMETA) || pressed.contains(&Key::KEY_RIGHTMETA)
        }
        _ => false,
    }
}

fn modifier_group_count(pressed: &HashSet<Key>) -> usize {
    let mut count = 0;
    if pressed.contains(&Key::KEY_LEFTCTRL) || pressed.contains(&Key::KEY_RIGHTCTRL) {
        count += 1;
    }
    if pressed.contains(&Key::KEY_LEFTALT) || pressed.contains(&Key::KEY_RIGHTALT) {
        count += 1;
    }
    if pressed.contains(&Key::KEY_LEFTSHIFT) || pressed.contains(&Key::KEY_RIGHTSHIFT) {
        count += 1;
    }
    if pressed.contains(&Key::KEY_LEFTMETA) || pressed.contains(&Key::KEY_RIGHTMETA) {
        count += 1;
    }
    count
}

fn target_modifier_group_count(target: &[Key]) -> usize {
    let mut count = 0;
    let mut has_ctrl = false;
    let mut has_alt = false;
    let mut has_shift = false;
    let mut has_meta = false;

    for k in target {
        match *k {
            Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => has_ctrl = true,
            Key::KEY_LEFTALT | Key::KEY_RIGHTALT => has_alt = true,
            Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => has_shift = true,
            Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => has_meta = true,
            _ => {}
        }
    }

    if has_ctrl { count += 1; }
    if has_alt { count += 1; }
    if has_shift { count += 1; }
    if has_meta { count += 1; }
    count
}

fn parse_hotkey(hotkey: &str) -> (Vec<Key>, Key) {
    let parts: Vec<&str> = hotkey.split('+').collect();
    if parts.is_empty() {
        return (Vec::new(), Key::KEY_SPACE);
    }

    let mut modifiers = Vec::new();
    let mut key = Key::KEY_SPACE;

    for part in parts {
        let part_lower = part.trim().to_lowercase();
        match part_lower.as_str() {
            "ctrl" | "control" => {
                modifiers.push(Key::KEY_LEFTCTRL);
            }
            "alt" | "option" => {
                modifiers.push(Key::KEY_LEFTALT);
            }
            "shift" => {
                modifiers.push(Key::KEY_LEFTSHIFT);
            }
            "super" | "meta" | "cmd" | "win" => {
                modifiers.push(Key::KEY_LEFTMETA);
            }
            "space" => key = Key::KEY_SPACE,
            "enter" | "return" => key = Key::KEY_ENTER,
            "tab" => key = Key::KEY_TAB,
            "escape" | "esc" => key = Key::KEY_ESC,
            "backspace" => key = Key::KEY_BACKSPACE,
            "delete" => key = Key::KEY_DELETE,
            "up" => key = Key::KEY_UP,
            "down" => key = Key::KEY_DOWN,
            "left" => key = Key::KEY_LEFT,
            "right" => key = Key::KEY_RIGHT,
            "f1" => key = Key::KEY_F1,
            "f2" => key = Key::KEY_F2,
            "f3" => key = Key::KEY_F3,
            "f4" => key = Key::KEY_F4,
            "f5" => key = Key::KEY_F5,
            "f6" => key = Key::KEY_F6,
            "f7" => key = Key::KEY_F7,
            "f8" => key = Key::KEY_F8,
            "f9" => key = Key::KEY_F9,
            "f10" => key = Key::KEY_F10,
            "f11" => key = Key::KEY_F11,
            "f12" => key = Key::KEY_F12,
            "a" => key = Key::KEY_A,
            "b" => key = Key::KEY_B,
            "c" => key = Key::KEY_C,
            "d" => key = Key::KEY_D,
            "e" => key = Key::KEY_E,
            "f" => key = Key::KEY_F,
            "g" => key = Key::KEY_G,
            "h" => key = Key::KEY_H,
            "i" => key = Key::KEY_I,
            "j" => key = Key::KEY_J,
            "k" => key = Key::KEY_K,
            "l" => key = Key::KEY_L,
            "m" => key = Key::KEY_M,
            "n" => key = Key::KEY_N,
            "o" => key = Key::KEY_O,
            "p" => key = Key::KEY_P,
            "q" => key = Key::KEY_Q,
            "r" => key = Key::KEY_R,
            "s" => key = Key::KEY_S,
            "t" => key = Key::KEY_T,
            "u" => key = Key::KEY_U,
            "v" => key = Key::KEY_V,
            "w" => key = Key::KEY_W,
            "x" => key = Key::KEY_X,
            "y" => key = Key::KEY_Y,
            "z" => key = Key::KEY_Z,
            "0" => key = Key::KEY_0,
            "1" => key = Key::KEY_1,
            "2" => key = Key::KEY_2,
            "3" => key = Key::KEY_3,
            "4" => key = Key::KEY_4,
            "5" => key = Key::KEY_5,
            "6" => key = Key::KEY_6,
            "7" => key = Key::KEY_7,
            "8" => key = Key::KEY_8,
            "9" => key = Key::KEY_9,
            _ => {}
        }
    }

    (modifiers, key)
}

fn launch_app(_config: &Config) {
    let exe_path = std::env::current_exe().unwrap_or_default();
    let app_path = exe_path.parent().unwrap_or(Path::new(".")).join("obsidian-launcher");

    info!("Launching app at: {:?}", app_path);

    match Command::new(&app_path).spawn() {
        Ok(_) => info!("App launched successfully"),
        Err(e) => warn!("Failed to launch app: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hotkey_super_space() {
        let (mods, key) = parse_hotkey("Super+Space");
        assert_eq!(mods, vec![Key::KEY_LEFTMETA]);
        assert_eq!(key, Key::KEY_SPACE);
    }

    #[test]
    fn test_parse_hotkey_ctrl_shift_f5() {
        let (mods, key) = parse_hotkey("Ctrl+Shift+F5");
        assert_eq!(mods, vec![Key::KEY_LEFTCTRL, Key::KEY_LEFTSHIFT]);
        assert_eq!(key, Key::KEY_F5);
    }

    #[test]
    fn test_parse_hotkey_just_key() {
        let (mods, key) = parse_hotkey("Space");
        assert!(mods.is_empty());
        assert_eq!(key, Key::KEY_SPACE);
    }

    #[test]
    fn test_parse_hotkey_empty_defaults_to_space() {
        let (mods, key) = parse_hotkey("");
        assert!(mods.is_empty());
        assert_eq!(key, Key::KEY_SPACE);
    }

    #[test]
    fn test_parse_hotkey_aliases() {
        let (mods, _key) = parse_hotkey("Control+Option+Meta");
        assert_eq!(mods, vec![Key::KEY_LEFTCTRL, Key::KEY_LEFTALT, Key::KEY_LEFTMETA]);
    }

    #[test]
    fn test_is_modifier_true() {
        assert!(is_modifier(Key::KEY_LEFTCTRL));
        assert!(is_modifier(Key::KEY_RIGHTCTRL));
        assert!(is_modifier(Key::KEY_LEFTALT));
        assert!(is_modifier(Key::KEY_LEFTMETA));
    }

    #[test]
    fn test_is_modifier_false() {
        assert!(!is_modifier(Key::KEY_A));
        assert!(!is_modifier(Key::KEY_SPACE));
        assert!(!is_modifier(Key::KEY_ENTER));
    }

    #[test]
    fn test_modifier_active_left_right() {
        let mut pressed = HashSet::new();
        pressed.insert(Key::KEY_LEFTCTRL);
        assert!(modifier_active(Key::KEY_RIGHTCTRL, &pressed));
    }

    #[test]
    fn test_modifier_active_missing() {
        let pressed = HashSet::new();
        assert!(!modifier_active(Key::KEY_LEFTMETA, &pressed));
    }

    #[test]
    fn test_modifier_group_count_none() {
        let pressed = HashSet::new();
        assert_eq!(modifier_group_count(&pressed), 0);
    }

    #[test]
    fn test_modifier_group_count_one() {
        let mut pressed = HashSet::new();
        pressed.insert(Key::KEY_LEFTCTRL);
        assert_eq!(modifier_group_count(&pressed), 1);
    }

    #[test]
    fn test_modifier_group_count_left_right_same_group() {
        let mut pressed = HashSet::new();
        pressed.insert(Key::KEY_LEFTCTRL);
        pressed.insert(Key::KEY_RIGHTCTRL);
        assert_eq!(modifier_group_count(&pressed), 1);
    }

    #[test]
    fn test_modifier_group_count_multiple_groups() {
        let mut pressed = HashSet::new();
        pressed.insert(Key::KEY_LEFTCTRL);
        pressed.insert(Key::KEY_LEFTALT);
        assert_eq!(modifier_group_count(&pressed), 2);
    }

    #[test]
    fn test_target_modifier_group_count() {
        assert_eq!(target_modifier_group_count(&[]), 0);
        assert_eq!(target_modifier_group_count(&[Key::KEY_LEFTCTRL]), 1);
        assert_eq!(target_modifier_group_count(&[Key::KEY_LEFTCTRL, Key::KEY_LEFTALT]), 2);
        assert_eq!(
            target_modifier_group_count(&[Key::KEY_LEFTCTRL, Key::KEY_LEFTALT, Key::KEY_LEFTMETA]),
            3
        );
    }
}
