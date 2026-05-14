# Obsidian Launcher

Un lanceur/recherche GUI pour Obsidian, inspiré par [Vicinae](https://docs.vicinae.com/).

## Features

- **GUI avec `iced`** - fenêtre avec recherche en temps réel
- Recherche full-text dans ton vault Obsidian via `tantivy`
- **File watcher** - re-indexation automatique quand une note change (`notify`)
- Navigation clavier (↑↓ Enter Esc)
- Preview des 3 premières lignes de chaque note
- Ouverture des notes via l'URI `obsidian://`
- Rebuild d'index avec `Ctrl+R`

## Installation

```bash
cargo build --release
```

Le binaire sera dans `target/release/obsidian-launcher`.

## Configuration

Le fichier de config est à `~/.config/obsidian-launcher/config.toml` :

```toml
vault_path = "/chemin/vers/ton/vault"
max_results = 50
hotkey = "Super+Space"
```

## Usage

```bash
./target/release/obsidian-launcher
```

### Contrôles

| Touche | Action |
|--------|--------|
| `↑` / `↓` | Naviguer les résultats |
| `Enter` | Ouvrir la note sélectionnée |
| `Esc` | Fermer l'app |
| `Ctrl+R` | Rebuild l'index |

### Hotkey global (Wayland/X11)

L'app inclut un **daemon** qui écoute le hotkey automatiquement via `evdev` (fonctionne sur X11 et Wayland).

**Installation automatique :**
```bash
chmod +x setup-daemon.sh
./setup-daemon.sh
```

**Manuellement :**
```bash
# 1. Build et installe le daemon
cargo build --release --bin obsidian-hotkey-daemon
cp target/release/obsidian-hotkey-daemon ~/.cargo/bin/

# 2. Configure le hotkey dans ~/.config/obsidian-launcher/config.toml
#    hotkey = "Super+Space"

# 3. Installe le service systemd
mkdir -p ~/.config/systemd/user/
cp obsidian-hotkey-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now obsidian-hotkey-daemon

# 4. Vérifie
systemctl --user status obsidian-hotkey-daemon
journalctl --user -u obsidian-hotkey-daemon -f
```

**Note** : Le daemon lit directement `/dev/input/event*` pour capturer les touches. Si tu as des problèmes de permission, ajoute-toi au groupe `input` :
```bash
sudo usermod -aG input $USER
```

## Architecture

- `vault.rs` - Scan et parsing des notes `.md`
- `index.rs` - Index full-text avec Tantivy + ngram tokenizer
- `config.rs` - Gestion de la configuration
- `watcher.rs` - File watcher avec debounce (2s)
- `hotkey_daemon.rs` - Daemon pour hotkey global via evdev
- `main.rs` - Application GUI `iced`

**Binaires :**
- `obsidian-launcher` - GUI de recherche
- `obsidian-hotkey-daemon` - Daemon background pour le hotkey

## TODO

- [ ] Fenêtre flottante sans bordure (style Spotlight) avec `gtk-layer-shell`
- [ ] Support des wikilinks `[[...]]` dans la recherche
- [ ] Package AppImage / deb
