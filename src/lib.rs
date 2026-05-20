pub mod config;
pub mod hotkey_daemon;
pub mod index;
pub mod vault;
mod watcher;

use std::path::PathBuf;
use std::process::Command;

use config::Config;
use index::SearchIndex;
use tokio::sync::mpsc;
use tracing::{info, warn};
use vault::Vault;
use watcher::{spawn_watcher_loop, VaultWatcher};

use std::sync::OnceLock;

use iced::widget::{
    button, column, container, horizontal_space, row, scrollable, svg, text,
    text_input, Column, Space,
};
use iced::widget::svg::Svg;
use iced::{
    event, keyboard, window, Element, Event, Length, Subscription, Task, Theme, Size,
};

const WINDOW_WIDTH: u32 = 700;
const WINDOW_HEIGHT: u32 = 500;

fn search_input_id() -> &'static text_input::Id {
    static ID: OnceLock<text_input::Id> = OnceLock::new();
    ID.get_or_init(|| text_input::Id::new("search-input"))
}

fn results_scroll_id() -> &'static scrollable::Id {
    static ID: OnceLock<scrollable::Id> = OnceLock::new();
    ID.get_or_init(|| scrollable::Id::new("results-scroll"))
}

const CARD_HEIGHT: f32 = 42.0;

const FOLDER_SVG: &[u8] = br##"<?xml version="1.0" ?>
<svg viewBox="0 0 91 91" xmlns="http://www.w3.org/2000/svg" fill="none">
<path d="M0.636,89.369h82.969c3.857,0,6.992-3.139,6.992-6.994V31.82H0.636V89.369z" fill="#647F94"/>
<path d="M47.745,11.9c-0.869,0-1.688-0.4-2.223-1.086l-7.254-9.299H0.632v24.666h89.965V11.9H47.745z" fill="#95AEC2"/>
</svg>"##;

const SETTINGS_SVG: &[u8] = br##"<svg viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg" fill="#000000">
<path fill-rule="evenodd" clip-rule="evenodd" fill="#BDC3C7" d="M97.55 85.718L45.407 33.574c-4.588-4.587 3.054-15.538-5.729-24.32L23.664 0l-3.381 3.38 8.832 8.831c3.381 3.38.849 10.983-2.545 14.377-3.367 3.367-10.977 5.906-14.357 2.525l-8.833-8.83L0 23.664l9.254 16.014c8.734 8.735 19.87 1.277 24.321 5.729l52.143 52.144A8.367 8.367 0 0 0 97.55 85.718zm-3.381 8.451a3.585 3.585 0 1 1-5.07-5.07 3.585 3.585 0 0 1 5.07 5.07z"/>
<path fill="#95A5A6" d="M33.682 12.334L22.512 1.151 20.283 3.38l8.832 8.831c3.381 3.38.849 10.983-2.545 14.377-3.367 3.367-10.977 5.906-14.357 2.525l-8.833-8.83-1.975 1.975 11.177 11.19c1.524 1.525 3.914 2.332 6.911 2.332 4.492 0 9.453-1.824 12.063-4.437 4.417-4.42 6.311-14.822 2.126-19.009zm62.064 75.615L45.775 37.972c-1.042-1.042-2.426-1.615-3.898-1.615s-2.857.574-3.898 1.615a5.522 5.522 0 0 0 0 7.798L87.95 95.746c1.041 1.042 2.426 1.615 3.898 1.615s2.857-.573 3.898-1.615a5.52 5.52 0 0 0 0-7.797zm-1.577 6.22a3.585 3.585 0 1 1-5.07-5.07 3.585 3.585 0 0 1 5.07 5.07z"/>
<path fill-rule="evenodd" clip-rule="evenodd" fill="#ECF0F1" d="M80 14L93 4l7 7-10 13h-5L55 54l-5-5 30-30v-5z"/>
<path fill-rule="evenodd" clip-rule="evenodd" fill="#BDC3C7" d="M52.5 51.5L55 54l30-30h5l10-13-3.5-3.5z"/>
<path fill-rule="evenodd" clip-rule="evenodd" fill="#BCA1F3" d="M42.51 46.095l.854.845L5.768 84.161a5.931 5.931 0 0 0 0 8.447l5.119 5.068c2.356 2.332 5.17 3.326 7.526.994l38.603-38.216.853.845c.942.933 2.471.933 3.413 0s.942-2.446 0-3.379L45.923 42.716c-.942-.933-2.471-.933-3.413 0s-.943 2.446 0 3.379z"/>
<path fill-rule="evenodd" clip-rule="evenodd" fill="#7146C7" d="M50.25 53.75L8.594 95.406l2.293 2.271c2.356 2.332 5.17 3.326 7.526.994l38.573-38.186-6.736-6.735z"/>
</svg>"##;

const LOGO_SVG: &[u8] = br##"<svg viewBox="0 0 397 512" fill="none" xmlns="http://www.w3.org/2000/svg">
<path d="M324.709 475.437C321.564 498.795 298.704 517.043 275.987 510.743C243.618 501.826 206.139 487.918 172.416 485.326C167.97 484.984 120.732 481.403 120.732 481.403C112.376 480.808 104.533 477.157 98.6965 471.148L9.63858 379.448C-0.0941202 369.428 -2.72861 354.484 2.99039 341.74C2.99039 341.74 58.0589 220.728 60.1042 214.435C62.1495 208.142 69.6555 153.256 74.1035 123.77C75.2825 115.956 79.1405 108.793 85.0165 103.508L190.369 8.74822C205.027 -4.43479 227.778 -2.4812 239.972 13.0074L328.473 125.419C333.48 131.781 336.047 139.688 336.084 147.783C336.185 169.08 337.943 212.805 349.719 240.968C361.175 268.361 382.2 297.946 393.189 312.504C397.405 318.091 398.052 325.646 394.489 331.673C386.734 344.801 371.412 370.009 349.719 402.351C334.764 424.651 327.834 452.218 324.709 475.437Z" fill="#6C31E3"/>
<path d="M108.293 478.079C149.651 394.115 148.498 333.963 130.899 291.034C114.702 251.53 84.578 226.613 60.834 211.149C60.334 213.383 59.609 215.564 58.67 217.658L2.98893 341.74C-2.73008 354.484 -0.0955851 369.428 9.63712 379.448L98.695 471.148C101.489 474.024 104.743 476.361 108.293 478.079Z" fill="url(#paint0_radial_1_115)"/>
<path d="M275.998 510.731C298.71 517.031 321.567 498.783 324.712 475.424C327.42 455.314 332.979 431.94 344.136 411.554C318.538 356.452 287.582 327.877 253.647 315.212C217.726 301.805 178.467 306.225 138.689 315.885C147.581 356.353 142.258 409.204 108.34 478.072C112.202 479.942 116.413 481.081 120.765 481.391C120.765 481.391 145.24 483.452 174.348 485.512C203.456 487.572 246.772 502.626 275.998 510.731Z" fill="url(#paint1_radial_1_115)"/>
<path d="M220.844 307.659C232.021 308.826 242.974 311.235 253.636 315.213C287.577 327.879 318.539 356.455 344.142 411.553C345.863 408.408 347.719 405.333 349.719 402.351C371.411 370.009 386.733 344.801 394.488 331.673C398.051 325.646 397.404 318.091 393.187 312.504C382.199 297.946 361.174 268.361 349.719 240.968C337.942 212.805 336.184 169.08 336.084 147.783C336.046 139.688 333.479 131.781 328.472 125.419L239.971 13.0073C239.498 12.4062 239.008 11.8256 238.504 11.2654C244.998 32.5466 244.558 49.6659 240.552 65.2287C236.838 79.6557 230.058 92.7451 222.897 106.571L222.896 106.573C220.493 111.21 218.049 115.93 215.662 120.812C206.161 140.247 197.582 162.241 196.317 191.734C195.051 221.228 201.097 258.22 220.847 307.658L220.844 307.659Z" fill="url(#paint2_radial_1_115)"/>
<path d="M220.832 307.658C201.084 258.221 195.035 221.228 196.301 191.733C197.567 162.238 206.146 140.244 215.649 120.807C218.036 115.924 220.481 111.204 222.884 106.566C230.044 92.7399 236.824 79.6517 240.537 65.226C244.544 49.6597 244.984 32.5357 238.484 11.2456C225.999 -2.61128 204.446 -3.91716 190.365 8.74822L85.0122 103.508C79.1362 108.793 75.2782 115.956 74.0992 123.77L61.2742 208.786C61.1552 209.575 61.0082 210.359 60.8342 211.138C84.5792 226.601 114.708 251.521 130.906 291.03C134.07 298.748 136.702 307.019 138.652 315.888C166.627 309.094 194.347 304.893 220.832 307.658Z" fill="url(#paint3_radial_1_115)"/>
<path fill-rule="evenodd" clip-rule="evenodd" d="M196.508 189.79C195.238 219.048 198.89 252.61 218.598 301.944L212.409 301.386C194.728 249.904 190.88 223.509 192.168 193.847C193.456 164.17 203.045 141.35 212.601 121.884C215.021 116.954 220.668 107.696 223.085 103.049C230.24 89.2908 235.002 82.0228 239.09 69.4468C244.804 51.8747 243.568 43.5519 242.917 35.2692C247.453 65.2105 230.234 91.2462 217.217 117.763C207.734 137.08 197.777 160.547 196.508 189.79Z" fill="url(#paint4_radial_1_115)"/>
<path fill-rule="evenodd" clip-rule="evenodd" d="M136.726 293.21C139.063 298.615 141.271 302.979 142.665 309.667L137.501 310.828C135.352 303.019 133.692 297.463 130.718 290.765C112.922 248.78 84.362 227.184 61.022 211.344C89.2138 226.511 118.149 250.251 136.726 293.21Z" fill="url(#paint5_radial_1_115)"/>
<path fill-rule="evenodd" clip-rule="evenodd" d="M142.965 314.949C152.827 360.838 141.826 419.137 109.406 475.807C136.504 419.644 149.641 365.703 138.7 315.864L142.965 314.949Z" fill="url(#paint6_radial_1_115)"/>
<path fill-rule="evenodd" clip-rule="evenodd" d="M254.86 310.821C308.014 330.713 328.48 374.389 343.778 410.822C324.885 372.671 298.618 330.538 252.953 314.9C218.207 303 188.862 304.411 138.697 315.796L137.579 310.821C190.819 298.691 218.655 297.272 254.86 310.821Z" fill="url(#paint7_radial_1_115)"/>
<defs>
<radialGradient id="paint0_radial_1_115" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(103.845 469.791) rotate(-104.574) scale(232.965 155.247)"><stop stop-color="white" stop-opacity="0.4"/><stop offset="1" stop-opacity="0.1"/></radialGradient>
<radialGradient id="paint1_radial_1_115" cx="0" cy="0" r="1" gradientTransform="matrix(-96.2576 -163.001 187.145 -110.545 277.685 511.988)" gradientUnits="userSpaceOnUse"><stop stop-color="white" stop-opacity="0.3"/><stop offset="1" stop-opacity="0.25"/></radialGradient>
<radialGradient id="paint2_radial_1_115" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(302.401 374) rotate(-82.4846) scale(382.284 282.434)"><stop stop-color="white" stop-opacity="0.55"/><stop offset="1" stop-color="white" stop-opacity="0.05"/></radialGradient>
<radialGradient id="paint3_radial_1_115" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(117.805 306.884) rotate(-77.7214) scale(326.45 222.631)"><stop stop-color="white" stop-opacity="0.83"/><stop offset="1" stop-color="white" stop-opacity="0.4"/></radialGradient>
<radialGradient id="paint4_radial_1_115" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(252.4 128) rotate(102.236) scale(169.859 114.542)"><stop stop-color="white" stop-opacity="0"/><stop offset="1" stop-color="white" stop-opacity="0.17"/></radialGradient>
<radialGradient id="paint5_radial_1_115" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(53.399 220) rotate(45.3237) scale(125.16 266.579)"><stop stop-color="white" stop-opacity="0.2"/><stop offset="1" stop-color="white" stop-opacity="0.44"/></radialGradient>
<radialGradient id="paint6_radial_1_115" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(147.891 279.224) rotate(80.2016) scale(146.696 311.515)"><stop stop-color="white" stop-opacity="0.25"/><stop offset="1" stop-color="white" stop-opacity="0.3"/></radialGradient>
<radialGradient id="paint7_radial_1_115" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(342.401 398.999) rotate(-152.297) scale(223.528 703.43)"><stop stop-color="white" stop-opacity="0.21"/><stop offset="0.46738" stop-color="white" stop-opacity="0.19"/><stop offset="1" stop-color="white" stop-opacity="0.29"/></radialGradient>
</defs>
</svg>"##;

const CLOSE_SVG: &[u8] = br##"<svg viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg">
<rect width="48" height="48" fill="white" fill-opacity="0.01"/>
<path d="M24 44C35.0457 44 44 35.0457 44 24C44 12.9543 35.0457 4 24 4C12.9543 4 4 12.9543 4 24C4 35.0457 12.9543 44 24 44Z" fill="#7146C7" stroke="#000000" stroke-width="4" stroke-linejoin="round"/>
<path d="M29.6569 18.3431L18.3432 29.6568" stroke="white" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
<path d="M18.3432 18.3431L29.6569 29.6568" stroke="white" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"##;

const NOTE_SVG: &[u8] = br##"<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" fill="none">
<rect x="2" y="2" width="20" height="20" rx="0"/>
<rect x="2" y="2" width="20" height="20" rx="0" fill="#7146C7" opacity="0.1"/>
<line x1="7" y1="8" x2="17" y2="8" stroke="#7146C7" stroke-width="1.5" stroke-linecap="round"/>
<line x1="7" y1="12" x2="17" y2="12" stroke="#7146C7" stroke-width="1.5" stroke-linecap="round"/>
<line x1="7" y1="16" x2="12" y2="16" stroke="#7146C7" stroke-width="1.5" stroke-linecap="round"/>
</svg>"##;

#[derive(Debug, Clone)]
enum Screen {
    Search,
    Settings,
}

#[derive(Debug, Clone)]
enum Message {
    SearchChanged(String),
    OpenSelected,
    OpenPath(String),
    SearchDone(Vec<SearchResult>),
    RebuildIndex,
    RebuildDone(bool),
    KeyPressed(keyboard::Key),
    Close,

    OpenSettings,
    CloseSettings,
    VaultPathChanged(String),
    MaxResultsChanged(String),
    StartRecordingHotkey,
    StopRecordingHotkey,
    RecordHotkey(keyboard::Key, String),
    FocusSearch,
    ScrollToSelected,
    SaveSettings,
    SettingsSaved(Result<(), String>),
    BrowseVault,
}

#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    path: String,
    folder: String,
    wikilinks: Vec<String>,
}

struct SettingsForm {
    vault_path: String,
    max_results: String,
    hotkey: String,
    saving: bool,
    saved: bool,
    error: Option<String>,
    recording_hotkey: bool,
    pending_modifiers: String,
}

impl SettingsForm {
    fn from_config(config: &Config) -> Self {
        Self {
            vault_path: config.vault_path.to_string_lossy().to_string(),
            max_results: config.max_results.to_string(),
            hotkey: config.hotkey.clone().unwrap_or_default(),
            saving: false,
            saved: false,
            error: None,
            recording_hotkey: false,
            pending_modifiers: String::new(),
        }
    }
}

struct State {
    screen: Screen,
    config: Config,
    search_query: String,
    results: Vec<SearchResult>,
    selected: usize,
    loading: bool,
    status: String,
    watcher: Option<VaultWatcher>,
    settings: SettingsForm,
}

impl Default for State {
    fn default() -> Self {
        let config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to load config: {}. Using defaults.", e);
                Config::default()
            }
        };
        let mut state = Self {
            screen: Screen::Search,
            config: config.clone(),
            search_query: String::new(),
            results: Vec::new(),
            selected: 0,
            loading: false,
            status: String::from("Initializing watcher..."),
            watcher: None,
            settings: SettingsForm::from_config(&config),
        };

        state.start_watcher();
        state
    }
}

impl State {
    fn start_watcher(&mut self) {
        let (watcher_tx, watcher_rx) = mpsc::channel(32);

        let config_clone = self.config.clone();
        let _ = spawn_watcher_loop(config_clone, watcher_rx);

        let mut watcher = VaultWatcher::new(self.config.clone(), watcher_tx);
        match watcher.start() {
            Ok(()) => {
                self.status = "Watching vault for changes".to_string();
            }
            Err(e) => {
                self.status = format!("Watcher error: {}", e);
            }
        }
        self.watcher = Some(watcher);
    }

    fn restart_watcher(&mut self) {
        self.watcher = None;
        self.start_watcher();
    }
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::SearchChanged(query) => {
            state.search_query = query.clone();
            state.selected = 0;

            if query.is_empty() {
                state.results.clear();
                state.status = String::from("Ready");
                return Task::none();
            }

            state.loading = true;
            state.status = format!("Searching '{}'...", query);

            let index_path = state.config.index_path();
            let max_results = state.config.max_results;

            info!("GUI search for: '{}'", query);

            Task::perform(
                async move {
                    info!("Async search starting for: '{}'", query);
                    let search_index = match SearchIndex::open_or_create(&index_path) {
                        Ok(si) => si,
                        Err(e) => {
                            info!("Failed to open index: {}", e);
                            return Vec::new();
                        }
                    };
                    let results = match search_index.search(&query, max_results) {
                        Ok(r) => r,
                        Err(e) => {
                            info!("Search failed: {}", e);
                            return Vec::new();
                        }
                    };
                    info!("Search returned {} results", results.len());

                    let mut search_results = Vec::new();
                    for (title, path, wikilinks) in results {
                        let folder = extract_folder(&path);
                        search_results.push(SearchResult {
                            title,
                            path,
                            folder,
                            wikilinks,
                        });
                    }
                    search_results
                },
                Message::SearchDone,
            )
        }

        Message::SearchDone(results) => {
            info!("SearchDone with {} results", results.len());
            state.results = results;
            state.loading = false;
            state.status = format!("{} results", state.results.len());
            Task::none()
        }

        Message::OpenSelected => {
            if state.selected < state.results.len() {
                let path = state.results[state.selected].path.clone();
                open_note(&path, &state.config.vault_path);
            }
            Task::done(Message::Close)
        }

        Message::OpenPath(path) => {
            info!("OpenPath: {}", path);
            open_note(&path, &state.config.vault_path);
            Task::done(Message::Close)
        }

        Message::RebuildIndex => {
            state.loading = true;
            state.status = String::from("Rebuilding index...");

            let config = state.config.clone();

            Task::perform(
                async move {
                    watcher::rebuild_vault_index(&config).ok()
                },
                |success| Message::RebuildDone(success.is_some()),
            )
        }

        Message::RebuildDone(success) => {
            state.loading = false;
            state.status = if success {
                String::from("Index rebuilt")
            } else {
                String::from("Index rebuild failed")
            };
            Task::none()
        }

        Message::KeyPressed(key) => {
            if state.settings.recording_hotkey {
                return Task::none();
            }

            if let Screen::Search = state.screen {
                match key {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        return Task::done(Message::Close);
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                        if state.selected > 0 {
                            state.selected -= 1;
                            return Task::done(Message::ScrollToSelected);
                        }
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                        if state.selected + 1 < state.results.len() {
                            state.selected += 1;
                            return Task::done(Message::ScrollToSelected);
                        }
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {}
                    _ => {}
                }
            } else if let Screen::Settings = state.screen {
                if matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape)) {
                    state.screen = Screen::Search;
                    state.settings = SettingsForm::from_config(&state.config);
                    return Task::none();
                }
            }
            Task::none()
        }

        Message::OpenSettings => {
            state.screen = Screen::Settings;
            state.settings = SettingsForm::from_config(&state.config);
            Task::none()
        }

        Message::CloseSettings => {
            state.screen = Screen::Search;
            state.settings = SettingsForm::from_config(&state.config);
            Task::none()
        }

        Message::VaultPathChanged(path) => {
            state.settings.vault_path = path;
            state.settings.saved = false;
            state.settings.error = None;
            Task::none()
        }

        Message::MaxResultsChanged(val) => {
            state.settings.max_results = val;
            state.settings.saved = false;
            state.settings.error = None;
            Task::none()
        }

        Message::StartRecordingHotkey => {
            state.settings.recording_hotkey = true;
            state.settings.pending_modifiers = String::new();
            Task::none()
        }

        Message::StopRecordingHotkey => {
            state.settings.recording_hotkey = false;
            state.settings.pending_modifiers = String::new();
            Task::none()
        }

        Message::RecordHotkey(key, mods) => {
            if !state.settings.recording_hotkey {
                return Task::none();
            }

            match key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    return Task::done(Message::StopRecordingHotkey);
                }
                keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                    return Task::done(Message::StopRecordingHotkey);
                }
                keyboard::Key::Named(keyboard::key::Named::Control)
                | keyboard::Key::Named(keyboard::key::Named::Alt)
                | keyboard::Key::Named(keyboard::key::Named::Shift)
                | keyboard::Key::Named(keyboard::key::Named::Super) => {
                    return Task::none();
                }
                _ => {}
            }

            let key_str = key_to_string(&key);
            let hotkey = if mods.is_empty() {
                key_str
            } else {
                format!("{}+{}", mods, key_str)
            };

            state.settings.hotkey = hotkey;
            state.settings.recording_hotkey = false;
            state.settings.pending_modifiers = String::new();
            state.settings.saved = false;
            Task::none()
        }

        Message::FocusSearch => {
            text_input::focus(search_input_id().clone())
        }

        Message::ScrollToSelected => {
            let anchor = state.selected.saturating_sub(5);
            let offset = anchor as f32 * CARD_HEIGHT;
            scrollable::scroll_to(results_scroll_id().clone(), iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: offset })
        }

        Message::SaveSettings => {
            let vault_path = state.settings.vault_path.clone();
            let max_results: usize = match state.settings.max_results.parse() {
                Ok(n) => n,
                Err(_) => {
                    state.settings.error = Some("Max results must be a number".to_string());
                    return Task::none();
                }
            };
            let hotkey = if state.settings.hotkey.is_empty() {
                None
            } else {
                Some(state.settings.hotkey.clone())
            };

            let new_config = Config {
                vault_path: PathBuf::from(&vault_path),
                index_path: None,
                max_results,
                hotkey,
            };

            state.settings.saving = true;

            Task::perform(
                async move {
                    if !new_config.vault_path.exists() {
                        return Err(format!("Vault not found at {:?}", new_config.vault_path));
                    }
                    new_config.save().map_err(|e| e.to_string())?;
                    Ok(())
                },
                Message::SettingsSaved,
            )
        }

        Message::SettingsSaved(result) => {
            state.settings.saving = false;
            match result {
                Ok(()) => {
                    state.settings.saved = true;
                    state.settings.error = None;

                    let old_vault = state.config.vault_path.clone();
                    match Config::load() {
                        Ok(c) => state.config = c,
                        Err(e) => {
                            state.settings.error = Some(format!("Failed to reload config: {}", e));
                            return Task::none();
                        }
                    }

                    if state.config.vault_path != old_vault {
                        info!("Vault path changed, rebuilding index...");
                        state.status = "New vault detected, rebuilding...".to_string();
                        state.restart_watcher();

                        let config = state.config.clone();
                        return Task::perform(
                            async move {
                                let vault = Vault::new(config.vault_path.clone());
                                let notes = vault.scan().map_err(|e| e.to_string())?;
                                let mut search_index =
                                    SearchIndex::open_or_create(&config.index_path())
                                        .map_err(|e| e.to_string())?;
                                search_index
                                    .index_notes(&notes)
                                    .map_err(|e| e.to_string())?;
                                Ok(())
                            },
                            |result: Result<(), String>| {
                                match result {
                                    Ok(()) => {
                                        info!("Index rebuilt after vault change");
                                    }
                                    Err(e) => {
                                        info!("Index rebuild failed: {}", e);
                                    }
                                }
                                Message::CloseSettings
                            },
                        );
                    } else {
                        state.restart_watcher();
                    }

                    if let Err(e) = std::process::Command::new("systemctl")
                        .args(["--user", "restart", "obsidian-hotkey-daemon"])
                        .status()
                    {
                        warn!("Failed to restart hotkey daemon: {}", e);
                    }
                    state.status = "Settings saved".to_string();
                }
                Err(e) => {
                    state.settings.error = Some(e);
                }
            }
            Task::none()
        }

        Message::BrowseVault => {
            let current = state.settings.vault_path.clone();
            let path = rfd::FileDialog::new()
                .set_directory(&current)
                .pick_folder();

            if let Some(path) = path {
                state.settings.vault_path = path.to_string_lossy().to_string();
            }
            Task::none()
        }

        Message::Close => {
            std::process::exit(0);
        }
    }
}

fn search_view(state: &State) -> Element<'_, Message> {
    let search_bar = text_input("Search notes or paste anything...", &state.search_query)
        .id(search_input_id().clone())
        .on_input(Message::SearchChanged)
        .on_submit(Message::OpenSelected)
        .size(20)
        .padding(14)
        .width(Length::Fill)
        .style(|_theme, _status| text_input::Style {
            background: iced::Background::Color(iced::Color::from_rgb8(50, 50, 53)),
            border: iced::Border {
                radius: 8.0.into(),
                width: 0.0,
                color: iced::Color::TRANSPARENT,
            },
            icon: iced::Color::from_rgb8(80, 80, 100),
            placeholder: iced::Color::from_rgb8(80, 80, 100),
            value: iced::Color::from_rgb8(200, 200, 210),
            selection: iced::Color::from_rgb8(60, 60, 80),
        });

    let header = row![
        text(&state.status).size(11)
            .style(|_theme| text::Style { color: Some(iced::Color::from_rgb8(80, 80, 100)) }),
        horizontal_space(),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    let mut col: Column<Message> = column![search_bar].spacing(8);

    if state.results.is_empty() && !state.search_query.is_empty() && !state.loading {
        col = col.push(
            container(text("No results found").size(13)
                .style(|_theme| text::Style { color: Some(iced::Color::from_rgb8(90, 90, 110)) }))
                .padding(16)
                .width(Length::Fill)
                .center_x(Length::Fill)
        );
    }

    if !state.results.is_empty() {
        let mut list = Column::new().spacing(2);

        for (i, result) in state.results.iter().enumerate() {
            let is_selected = i == state.selected;

            let mut row_children: Vec<Element<'_, Message>> = vec![
                Svg::new(svg::Handle::from_memory(NOTE_SVG))
                    .width(16)
                    .height(16)
                    .into(),
                text(strip_emoji(&result.title)).size(14)
                    .style(move |_theme| text::Style {
                        color: Some(if is_selected { iced::Color::from_rgb8(255, 255, 255) } else { iced::Color::from_rgb8(210, 210, 220) }),
                    })
                    .into(),
                horizontal_space().into(),
            ];

            // Add wikilink indicator if the note has outgoing wikilinks
            if !result.wikilinks.is_empty() {
                let wikilink_count = result.wikilinks.len();
                row_children.push(
                    text(format!("[[{}]]", wikilink_count)).size(9)
                        .style(|_theme| text::Style { color: Some(iced::Color::from_rgb8(113, 70, 199)) })
                        .into()
                );
            }

            row_children.push(
                Svg::new(svg::Handle::from_memory(FOLDER_SVG))
                    .width(12)
                    .height(12)
                    .into()
            );
            row_children.push(
                text(format!(" {}", result.folder)).size(10)
                    .style(|_theme| text::Style { color: Some(iced::Color::from_rgb8(120, 120, 140)) })
                    .into()
            );

            let row_content = row(row_children)
            .align_y(iced::Alignment::Center)
            .spacing(8);

            let item = button(
                    container(row_content).padding(10).width(Length::Fill)
                )
                .on_press(Message::OpenPath(result.path.clone()))
                .padding(0)
                .width(Length::Fill)
                .style(move |_theme, _status| {
                    if is_selected {
                        button::Style {
                            background: Some(iced::Color::from_rgb8(50, 50, 53).into()),
                            text_color: iced::Color::WHITE,
                            border: iced::Border { radius: 6.0.into(), width: 0.0, color: iced::Color::TRANSPARENT },
                            shadow: iced::Shadow::default(),
                        }
                    } else {
                        button::Style {
                            background: Some(iced::Color::from_rgb8(35, 35, 37).into()),
                            text_color: iced::Color::WHITE,
                            border: iced::Border { radius: 6.0.into(), width: 0.0, color: iced::Color::TRANSPARENT },
                            shadow: iced::Shadow::default(),
                        }
                    }
                });

            list = list.push(item);
        }

        col = col.push(
            scrollable(list)
                .id(results_scroll_id().clone())
                .height(Length::Fill)
                .style(|_theme, _status| scrollable::Style {
                    container: container::Style::default(),
                    vertical_rail: scrollable::Rail {
                        background: None,
                        border: iced::Border { radius: 0.0.into(), width: 0.0, color: iced::Color::TRANSPARENT },
                        scroller: scrollable::Scroller {
                            color: iced::Color::from_rgba(0.5, 0.5, 0.6, 0.3),
                            border: iced::Border { radius: 4.0.into(), width: 1.0, color: iced::Color::from_rgba(0.5, 0.5, 0.6, 0.1) },
                        },
                    },
                    horizontal_rail: scrollable::Rail {
                        background: None,
                        border: iced::Border { radius: 0.0.into(), width: 0.0, color: iced::Color::TRANSPARENT },
                        scroller: scrollable::Scroller {
                            color: iced::Color::from_rgba(0.5, 0.5, 0.6, 0.3),
                            border: iced::Border { radius: 4.0.into(), width: 1.0, color: iced::Color::from_rgba(0.5, 0.5, 0.6, 0.1) },
                        },
                    },
                    gap: None,
                })
        );
    }

    let footer = row![
        horizontal_space(),
        button(
            Svg::new(svg::Handle::from_memory(SETTINGS_SVG))
                .width(16)
                .height(16)
        )
            .on_press(Message::OpenSettings)
            .padding(6)
            .style(|_theme, _status| button::Style {
                background: Some(iced::Color::from_rgb8(35, 35, 40).into()),
                text_color: iced::Color::from_rgb8(120, 120, 140),
                border: iced::Border { radius: 6.0.into(), width: 0.0, color: iced::Color::TRANSPARENT },
                shadow: iced::Shadow::default(),
            }),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    let logo = Svg::new(svg::Handle::from_memory(LOGO_SVG))
        .width(24)
        .height(32);

    container(column![
            container(row![logo, header].spacing(8).align_y(iced::Alignment::Center)).padding([16.0, 16.0]).width(Length::Fill),
            container(col).padding([0.0, 16.0]).width(Length::Fill),
            container(footer).padding([8.0, 16.0]).width(Length::Fill),
        ].spacing(6))
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_t| container::Style {
            background: Some(iced::Color::from_rgb8(35, 35, 37).into()),
            border: iced::Border {
                radius: 8.0.into(),
                width: 0.0,
                color: iced::Color::TRANSPARENT,
            },
            ..Default::default()
        })
        .into()
}

fn settings_view(state: &State) -> Element<'_, Message> {
    let s = &state.settings;

    let vault_row = row![
        text("Vault path").width(Length::Fixed(120.0)),
        text_input("Path to vault", &s.vault_path)
            .on_input(Message::VaultPathChanged)
            .width(Length::Fill),
        button(text("Browse").size(13))
            .on_press(Message::BrowseVault)
            .padding(8)
            .style(|_theme, _status| button::Style {
                background: Some(iced::Color::from_rgb8(55, 55, 55).into()),
                text_color: iced::Color::WHITE,
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: iced::Color::from_rgb8(80, 80, 80),
                },
                shadow: iced::Shadow::default(),
            }),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let max_results_row = row![
        text("Max results").width(Length::Fixed(120.0)),
        text_input("50", &s.max_results)
            .on_input(Message::MaxResultsChanged)
            .width(Length::Fixed(100.0)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let hotkey_display = if s.recording_hotkey {
        if s.pending_modifiers.is_empty() {
            String::from("Listening...")
        } else {
            format!("Listening... ({})", s.pending_modifiers)
        }
    } else if s.hotkey.is_empty() {
        String::from("None")
    } else {
        s.hotkey.clone()
    };

    let hotkey_btn_style = if s.recording_hotkey {
        button::Style {
            background: Some(iced::Color::from_rgb8(180, 120, 0).into()),
            text_color: iced::Color::WHITE,
            border: iced::Border {
                radius: 6.0.into(),
                width: 2.0,
                color: iced::Color::from_rgb8(255, 180, 0),
            },
            shadow: iced::Shadow::default(),
        }
    } else {
        button::Style {
            background: Some(iced::Color::from_rgb8(55, 55, 55).into()),
            text_color: iced::Color::WHITE,
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgb8(80, 80, 80),
            },
            shadow: iced::Shadow::default(),
        }
    };

    let mut hotkey_btn = button(text(hotkey_display.clone()).size(14))
        .padding(10)
        .width(Length::Fixed(200.0))
        .style(move |_theme, _status| hotkey_btn_style);

    if s.recording_hotkey {
        hotkey_btn = hotkey_btn.on_press(Message::StopRecordingHotkey);
    } else {
        hotkey_btn = hotkey_btn.on_press(Message::StartRecordingHotkey);
    }

    let hotkey_row = row![
        text("Hotkey").width(Length::Fixed(120.0)),
        hotkey_btn,
        text(if s.recording_hotkey {
            "Press your shortcut (Esc to cancel)"
        } else {
            "Click to record"
        })
        .size(11),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let mut save_btn = button(text(if s.saving { "Saving..." } else { "Save" }).size(14))
        .padding(10)
        .style(|_theme, _status| button::Style {
            background: Some(iced::Color::from_rgb8(113, 70, 199).into()),
            text_color: iced::Color::WHITE,
            border: iced::Border {
                radius: 6.0.into(),
                width: 0.0,
                color: iced::Color::TRANSPARENT,
            },
            shadow: iced::Shadow::default(),
        });

    if !s.saving {
        save_btn = save_btn.on_press(Message::SaveSettings);
    }

    let cancel_btn = button(text("Cancel").size(14))
        .on_press(Message::CloseSettings)
        .padding(10)
        .style(|_theme, _status| button::Style {
            background: Some(iced::Color::from_rgb8(55, 55, 55).into()),
            text_color: iced::Color::WHITE,
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgb8(80, 80, 80),
            },
            shadow: iced::Shadow::default(),
        });

    let buttons = row![save_btn, cancel_btn].spacing(10);

    let title_row = row![
        text("Settings").size(20),
        horizontal_space(),
        button(
            Svg::new(svg::Handle::from_memory(CLOSE_SVG))
                .width(18)
                .height(18)
        )
            .on_press(Message::CloseSettings)
            .padding(6)
            .style(|_theme, _status| button::Style {
                background: Some(iced::Color::from_rgb8(45, 45, 50).into()),
                text_color: iced::Color::from_rgb8(150, 150, 160),
                border: iced::Border { radius: 6.0.into(), width: 0.0, color: iced::Color::TRANSPARENT },
                shadow: iced::Shadow::default(),
            }),
    ]
    .align_y(iced::Alignment::Center);

    let mut form = column![
        title_row,
        Space::with_height(10),
        vault_row,
        Space::with_height(10),
        max_results_row,
        Space::with_height(10),
        hotkey_row,
        Space::with_height(20),
        buttons,
    ]
    .spacing(5);

    if s.saved {
        form = form.push(Space::with_height(10));
        form = form.push(text("✓ Settings saved!").style(|_theme| text::Style {
            color: Some(iced::Color::from_rgb8(34, 197, 34)),
        }));
    }

    if let Some(ref err) = s.error {
        form = form.push(Space::with_height(10));
        form = form.push(text(format!("✗ {}", err)).style(|_theme| text::Style {
            color: Some(iced::Color::from_rgb8(239, 68, 68)),
        }));
    }

    container(scrollable(form).height(Length::Fill))
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view(state: &State) -> Element<'_, Message> {
    match state.screen {
        Screen::Search => search_view(state),
        Screen::Settings => settings_view(state),
    }
}

fn subscription(state: &State) -> Subscription<Message> {
    let startup_sub = Subscription::run_with_id(
        "startup-focus",
        iced::futures::stream::once(async { Message::FocusSearch }),
    );

    let recording_sub = if state.settings.recording_hotkey {
        Some(event::listen_with(|event, _status, _window_id| {
            match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    match key {
                        keyboard::Key::Named(keyboard::key::Named::Escape)
                        | keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                            Some(Message::StopRecordingHotkey)
                        }
                        keyboard::Key::Named(keyboard::key::Named::Control)
                        | keyboard::Key::Named(keyboard::key::Named::Alt)
                        | keyboard::Key::Named(keyboard::key::Named::Shift)
                        | keyboard::Key::Named(keyboard::key::Named::Super) => {
                            None
                        }
                        _ => {
                            let mod_str = build_modifier_string(&modifiers);
                            Some(Message::RecordHotkey(key, mod_str))
                        }
                    }
                }
                _ => None,
            }
        }))
    } else {
        None
    };

    let input_sub = event::listen_with(|event, _status, _window_id| {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if modifiers.control() && key == keyboard::Key::Character(",".into()) {
                    Some(Message::OpenSettings)
                } else if modifiers.control() && key == keyboard::Key::Character("r".into()) {
                    Some(Message::RebuildIndex)
                } else if matches!(key, keyboard::Key::Named(keyboard::key::Named::ArrowUp))
                    || matches!(key, keyboard::Key::Named(keyboard::key::Named::ArrowDown))
                    || matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape))
                    || matches!(key, keyboard::Key::Named(keyboard::key::Named::Enter))
                {
                    Some(Message::KeyPressed(key))
                } else {
                    None
                }
            }
            _ => None,
        }
    });

    let mut subs = vec![startup_sub, input_sub];
    if let Some(s) = recording_sub {
        subs.push(s);
    }

    Subscription::batch(subs)
}

fn build_modifier_string(modifiers: &keyboard::Modifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.control() {
        parts.push("Ctrl");
    }
    if modifiers.alt() {
        parts.push("Alt");
    }
    if modifiers.shift() {
        parts.push("Shift");
    }
    if modifiers.logo() {
        parts.push("Super");
    }
    parts.join("+")
}

fn key_to_string(key: &keyboard::Key) -> String {
    match key {
        keyboard::Key::Named(n) => match n {
            keyboard::key::Named::Space => "Space".to_string(),
            keyboard::key::Named::Enter => "Enter".to_string(),
            keyboard::key::Named::Tab => "Tab".to_string(),
            keyboard::key::Named::Escape => "Esc".to_string(),
            keyboard::key::Named::Backspace => "Backspace".to_string(),
            keyboard::key::Named::Delete => "Delete".to_string(),
            keyboard::key::Named::ArrowUp => "Up".to_string(),
            keyboard::key::Named::ArrowDown => "Down".to_string(),
            keyboard::key::Named::ArrowLeft => "Left".to_string(),
            keyboard::key::Named::ArrowRight => "Right".to_string(),
            keyboard::key::Named::F1 => "F1".to_string(),
            keyboard::key::Named::F2 => "F2".to_string(),
            keyboard::key::Named::F3 => "F3".to_string(),
            keyboard::key::Named::F4 => "F4".to_string(),
            keyboard::key::Named::F5 => "F5".to_string(),
            keyboard::key::Named::F6 => "F6".to_string(),
            keyboard::key::Named::F7 => "F7".to_string(),
            keyboard::key::Named::F8 => "F8".to_string(),
            keyboard::key::Named::F9 => "F9".to_string(),
            keyboard::key::Named::F10 => "F10".to_string(),
            keyboard::key::Named::F11 => "F11".to_string(),
            keyboard::key::Named::F12 => "F12".to_string(),
            keyboard::key::Named::Insert => "Insert".to_string(),
            keyboard::key::Named::Home => "Home".to_string(),
            keyboard::key::Named::End => "End".to_string(),
            keyboard::key::Named::PageUp => "PageUp".to_string(),
            keyboard::key::Named::PageDown => "PageDown".to_string(),
            _ => format!("{:?}", n).replace("Named::", "").replace("Key", ""),
        },
        keyboard::Key::Character(c) => {
            let s = c.to_string();
            if s.len() == 1 {
                s.to_uppercase()
            } else {
                s
            }
        }
        _ => String::from("Unknown"),
    }
}

fn theme(_state: &State) -> Theme {
    Theme::Dark
}

fn focus_obsidian() {
    if let Ok(_) = Command::new("xdotool")
        .args(["search", "--name", "Obsidian", "windowactivate"])
        .status()
    {}
}

fn encode_path(s: &str) -> String {
    s.split('/')
        .map(|part| urlencoding::encode(part))
        .collect::<Vec<_>>()
        .join("/")
}

fn open_note(path: &str, vault_path: &PathBuf) {
    info!("Opening note: {}", path);

    let relative = path
        .strip_prefix(vault_path.to_string_lossy().as_ref())
        .map(|r| r.strip_prefix('/').unwrap_or(r))
        .unwrap_or(path);
    let url = format!("obsidian://open?file={}", encode_path(relative));

    info!("Opening URL: {}", url);
    if let Err(e) = Command::new("xdg-open").arg(&url).status() {
        warn!("Failed to open note: {}", e);
    }

    focus_obsidian();
}

fn strip_emoji(s: &str) -> String {
    s.chars().filter(|c| !matches!(c,
        '\u{1F300}'..='\u{1F9FF}' |
        '\u{2600}'..='\u{27BF}' |
        '\u{FE00}'..='\u{FE0F}' |
        '\u{200D}' | '\u{20E3}' |
        '\u{00A9}' | '\u{00AE}' | '\u{2122}' |
        '\u{23F0}' | '\u{23F3}'
    )).collect::<String>().trim().to_string()
}

fn extract_folder(path: &str) -> String {
    let p = std::path::Path::new(path);
    p.parent()
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn run_app() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}. Using defaults.", e);
            Config::default()
        }
    };

    if !config.vault_path.exists() {
        eprintln!("Vault not found at {:?}", config.vault_path);
        eprintln!("Edit config at ~/.config/obsidian-launcher/config.toml");
        std::process::exit(1);
    }

    if !config.index_path().exists() {
        info!("No index found, building initial index...");
        let vault = Vault::new(config.vault_path.clone());
        match vault.scan() {
            Ok(notes) => {
                match SearchIndex::open_or_create(&config.index_path()) {
                    Ok(mut search_index) => {
                        if let Err(e) = search_index.index_notes(&notes) {
                            eprintln!("Failed to build initial index: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to create search index: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to scan vault: {}. Start the app anyway?", e);
            }
        }
    }

    iced::application("Obsidian Launcher", update, view)
        .subscription(subscription)
        .theme(theme)
        .window(window::Settings {
            size: Size::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32),
            min_size: Some(Size::new(400.0, 300.0)),
            resizable: true,
            decorations: false,
            transparent: true,
            ..Default::default()
        })
        .run()
}
