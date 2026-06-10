use std::time::Duration;

use anyhow::Result;
use notify::{
    event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::Config;
use crate::index::SearchIndex;
use crate::vault::Vault;

pub enum WatcherEvent {
    VaultChanged,
}

pub struct VaultWatcher {
    config: Config,
    sender: mpsc::Sender<WatcherEvent>,
    _watcher: Option<RecommendedWatcher>,
}

impl VaultWatcher {
    pub fn new(config: Config, sender: mpsc::Sender<WatcherEvent>) -> Self {
        Self {
            config,
            sender,
            _watcher: None,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        let vault_path = self.config.vault_path.clone();
        let sender = self.sender.clone();

        let mut watcher: RecommendedWatcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                match res {
                    Ok(event) => {
                        if matches!(
                            event.kind,
                            EventKind::Modify(ModifyKind::Data(_))
                                | EventKind::Create(_)
                                | EventKind::Remove(_)
                        ) {
                            if event.paths.iter().any(|p| {
                                p.extension()
                                    .map_or(false, |ext| ext == "md" || ext == "MD")
                            }) {
                                info!("Vault change detected: {:?}", event.paths);
                                if let Err(e) = sender.blocking_send(WatcherEvent::VaultChanged) {
                                    warn!("Failed to send watcher event (channel full?): {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => warn!("Watch error: {:?}", e),
                }
            })?;

        watcher.watch(&vault_path, RecursiveMode::Recursive)?;
        info!("Watching vault at {:?}", vault_path);

        self._watcher = Some(watcher);
        Ok(())
    }
}

pub fn rebuild_vault_index(config: &Config) -> Result<()> {
    info!("Rebuilding vault index...");
    let vault = Vault::new(config.vault_path.clone());
    let notes = vault.scan()?;

    let mut search_index = SearchIndex::open_or_create(&config.index_path())?;
    search_index.index_notes(&notes)?;

    info!("Index rebuilt successfully");
    Ok(())
}

pub fn spawn_watcher_loop(
    config: Config,
    mut receiver: mpsc::Receiver<WatcherEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut pending_rebuild = false;
        let mut debounce_timer = tokio::time::interval(Duration::from_secs(2));
        debounce_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        debounce_timer.tick().await;

        // Use a flag to track whether the channel is still alive.
        let mut channel_alive = true;

        while channel_alive {
            tokio::select! {
                event = receiver.recv() => {
                    match event {
                        Some(WatcherEvent::VaultChanged) => {
                            pending_rebuild = true;
                            debounce_timer.reset();
                        }
                        None => {
                            warn!("Watcher channel closed — stopping watcher loop");
                            channel_alive = false;
                        }
                    }
                }
                _ = debounce_timer.tick() => {
                    if pending_rebuild {
                        pending_rebuild = false;
                        if let Err(e) = rebuild_vault_index(&config) {
                            warn!("Failed to rebuild index: {}", e);
                        }
                    }
                }
            }
        }

        info!("Watcher loop stopped");
    })
}
