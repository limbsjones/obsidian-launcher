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
use tracing::info;
use vault::Vault;
use watcher::{spawn_watcher_loop, VaultWatcher, WatcherEvent};

use iced::widget::{
    button, column, container, horizontal_space, row, scrollable, text,
    text_input, Column, Space,
};
use iced::{
    event, keyboard, window, Element, Event, Length, Subscription, Task, Theme, Size,
};

const WINDOW_WIDTH: u32 = 700;
const WINDOW_HEIGHT: u32 = 500;

#[derive(Debug, Clone)]
enum Screen {
    Search,
    Settings,
}

#[derive(Debug, Clone)]
enum Message {
    SearchChanged(String),
    OpenSelected,
    SearchDone(Vec<SearchResult>),
    RebuildIndex,
    RebuildDone,
    KeyPressed(keyboard::Key),
    Close,

    OpenSettings,
    CloseSettings,
    VaultPathChanged(String),
    MaxResultsChanged(String),
    HotkeyChanged(String),
    StartRecordingHotkey,
    StopRecordingHotkey,
    RecordHotkey(keyboard::Key, String),
    SaveSettings,
    SettingsSaved(Result<(), String>),
    BrowseVault,
}

#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    path: String,
    preview: String,
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
    watcher_tx: Option<mpsc::Sender<WatcherEvent>>,
    settings: SettingsForm,
}

impl Default for State {
    fn default() -> Self {
        let config = Config::load().expect("Failed to load config");
        let mut state = Self {
            screen: Screen::Search,
            config: config.clone(),
            search_query: String::new(),
            results: Vec::new(),
            selected: 0,
            loading: false,
            status: String::from("Initializing watcher..."),
            watcher_tx: None,
            settings: SettingsForm::from_config(&config),
        };

        state.start_watcher();
        state
    }
}

impl State {
    fn start_watcher(&mut self) {
        let (watcher_tx, watcher_rx) = mpsc::channel(32);
        self.watcher_tx = Some(watcher_tx.clone());

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
    }

    fn restart_watcher(&mut self) {
        self.watcher_tx = None;
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
                    for (title, path) in results {
                        let preview = read_preview(&path);
                        search_results.push(SearchResult {
                            title,
                            path,
                            preview,
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
                let path = &state.results[state.selected].path;
                open_note(path);
                state.search_query = String::new();
                state.results.clear();
                state.selected = 0;
                state.status = String::from("Ready");
            }
            Task::none()
        }

        Message::RebuildIndex => {
            state.loading = true;
            state.status = String::from("Rebuilding index...");

            let config = state.config.clone();

            Task::perform(
                async move {
                    let _ = watcher::rebuild_vault_index(&config);
                },
                |_| Message::RebuildDone,
            )
        }

        Message::RebuildDone => {
            state.loading = false;
            state.status = String::from("Index rebuilt");
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
                        }
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                        if state.selected + 1 < state.results.len() {
                            state.selected += 1;
                        }
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {
                        if state.selected < state.results.len() {
                            let path = &state.results[state.selected].path;
                            open_note(path);
                            state.search_query = String::new();
                            state.results.clear();
                            state.selected = 0;
                            state.status = String::from("Ready");
                        }
                    }
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

        Message::HotkeyChanged(val) => {
            state.settings.hotkey = val;
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
                    state.config = Config::load().expect("Failed to reload config");

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
                        state.status = "Settings saved".to_string();
                    }
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
    let settings_btn = button(text("⚙").size(16))
        .on_press(Message::OpenSettings)
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
        });

    let search_bar = text_input("Search notes...", &state.search_query)
        .on_input(Message::SearchChanged)
        .on_submit(Message::OpenSelected)
        .size(18)
        .padding(12)
        .width(Length::Fill);

    let header = row![
        text("Obsidian Launcher").size(16),
        horizontal_space(),
        text(&state.status).size(12),
        settings_btn,
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let mut content: Column<Message> = column![header, search_bar].spacing(10);

    if state.loading {
        content = content.push(text("Loading...").size(14));
    } else if state.results.is_empty() && !state.search_query.is_empty() {
        content = content.push(text("No results found").size(14));
    } else {
        let mut results_col = Column::new().spacing(2);

        for (i, result) in state.results.iter().enumerate() {
            let is_selected = i == state.selected;

            let title_text = text(&result.title).size(15);
            let preview_text = text(&result.preview).size(11);

            let item = column![title_text, preview_text].spacing(4);

            let row_item = row![item]
                .padding(10)
                .width(Length::Fill)
                .align_y(iced::Alignment::Center);

            if is_selected {
                results_col = results_col.push(
                    container(row_item)
                        .style(|_theme| container::Style {
                            background: Some(iced::Color::from_rgb8(59, 130, 246).into()),
                            ..Default::default()
                        })
                        .width(Length::Fill),
                );
            } else {
                results_col = results_col.push(row_item);
            }
        }

        content = content.push(scrollable(results_col).height(Length::Fill));
    }

    let footer = row![
        text("Up/Down Navigate").size(11),
        text("Enter Open").size(11),
        text("Esc Close").size(11),
        text("Ctrl+R Rebuild").size(11),
        text("Ctrl+, Settings").size(11),
    ]
    .spacing(15);

    container(column![content, footer].spacing(10))
        .padding(15)
        .width(Length::Fill)
        .height(Length::Fill)
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
            background: Some(iced::Color::from_rgb8(34, 139, 34).into()),
            text_color: iced::Color::WHITE,
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: iced::Color::from_rgb8(80, 80, 80),
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

    let mut form = column![
        text("Settings").size(20),
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
    let search_sub = keyboard::on_key_press(|key, modifiers| {
        if modifiers.control() && key == keyboard::Key::Character(",".into()) {
            Some(Message::OpenSettings)
        } else if modifiers.control() && key == keyboard::Key::Character("r".into()) {
            Some(Message::RebuildIndex)
        } else {
            Some(Message::KeyPressed(key))
        }
    });

    if state.settings.recording_hotkey {
        let recording_sub = event::listen_with(|event, _status, _window_id| {
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
        });
        Subscription::batch([search_sub, recording_sub])
    } else {
        search_sub
    }
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

fn open_note(path: &str) {
    info!("Opening note: {}", path);
    let obsidian_url = format!("obsidian://open?path={}", path);
    let _ = Command::new("xdg-open").arg(&obsidian_url).status();
}

fn read_preview(path: &str) -> String {
    if let Ok(content) = std::fs::read_to_string(path) {
        let lines: Vec<&str> = content.lines().collect();
        let preview: String = lines
            .iter()
            .take(3)
            .map(|l| l.trim_start_matches('#').trim())
            .collect::<Vec<_>>()
            .join(" ");
        if preview.len() > 100 {
            preview.chars().take(100).collect::<String>() + "..."
        } else {
            preview
        }
    } else {
        String::new()
    }
}

pub fn run_app() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config = Config::load().expect("Failed to load config");

    if !config.vault_path.exists() {
        eprintln!("Vault not found at {:?}", config.vault_path);
        eprintln!("Edit config at ~/.config/obsidian-launcher/config.toml");
        std::process::exit(1);
    }

    if !config.index_path().exists() {
        info!("No index found, building initial index...");
        let vault = Vault::new(config.vault_path.clone());
        if let Ok(notes) = vault.scan() {
            let mut search_index =
                SearchIndex::open_or_create(&config.index_path()).expect("Failed to create index");
            let _ = search_index.index_notes(&notes);
        }
    }

    iced::application("Obsidian Launcher", update, view)
        .subscription(subscription)
        .theme(theme)
        .window(window::Settings {
            size: Size::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32),
            min_size: Some(Size::new(400.0, 300.0)),
            resizable: true,
            decorations: true,
            ..Default::default()
        })
        .run()
}
