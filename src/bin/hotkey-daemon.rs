use obsidian_launcher::config::Config;
use obsidian_launcher::hotkey_daemon::HotkeyDaemon;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config = Config::load().expect("Failed to load config");

    if config.hotkey.is_none() || config.hotkey.as_ref().map_or(true, |h| h.is_empty()) {
        eprintln!("No hotkey configured. Set it in ~/.config/obsidian-launcher/config.toml");
        std::process::exit(1);
    }

    let mut daemon = HotkeyDaemon::new(config);
    daemon.run();
}
