use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use evdev::{Device, EventType, InputEventKind, Key};
use tracing::{info, warn};

use crate::config::Config;

pub struct HotkeyDaemon {
    config: Config,
    running: bool,
}

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

        let devices = self.find_keyboard_devices();
        if devices.is_empty() {
            warn!("No keyboard devices found");
            return;
        }

        info!("Found {} keyboard device(s)", devices.len());

        let hotkey = self.config.hotkey.clone().unwrap_or_default();
        let (target_modifiers, target_key) = parse_hotkey(&hotkey);

        info!("Parsed hotkey: modifiers={:?}, key={:?}", target_modifiers, target_key);

        let mut active_modifiers: HashMap<Key, bool> = HashMap::new();

        for mut device in devices {
            let dev_name = device.name().unwrap_or("unknown").to_string();
            info!("Listening on keyboard: {}", dev_name);

            let target_mods_clone = target_modifiers.clone();
            let target_key_clone = target_key;
            let config_clone = self.config.clone();

            thread::spawn(move || {
                let mut pressed_modifiers: HashMap<Key, bool> = HashMap::new();

                loop {
                    let events = match device.fetch_events() {
                        Ok(events) => events,
                        Err(e) => {
                            warn!("Failed to fetch events from {}: {}", dev_name, e);
                            thread::sleep(Duration::from_millis(100));
                            continue;
                        }
                    };

                    for event in events {
                        if event.event_type() != EventType::KEY {
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
                                pressed_modifiers.insert(key, true);
                            }

                            if key == target_key_clone {
                                let mods_match = target_mods_clone.iter().all(|m| {
                                    pressed_modifiers.get(m).copied().unwrap_or(false)
                                });

                                if mods_match && !target_mods_clone.is_empty() || (target_mods_clone.is_empty() && pressed_modifiers.is_empty()) {
                                    info!("Hotkey detected! Launching app...");
                                    launch_app(&config_clone);
                                }
                            }
                        } else if is_release {
                            pressed_modifiers.remove(&key);
                        }
                    }
                }
            });
        }

        while self.running {
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn find_keyboard_devices(&self) -> Vec<Device> {
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
                        devices.push(device);
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

fn launch_app(config: &Config) {
    let exe_path = std::env::current_exe().unwrap_or_default();
    let app_path = exe_path.parent().unwrap_or(Path::new(".")).join("obsidian-launcher");

    info!("Launching app at: {:?}", app_path);

    match Command::new(&app_path).spawn() {
        Ok(_) => info!("App launched successfully"),
        Err(e) => warn!("Failed to launch app: {}", e),
    }
}
