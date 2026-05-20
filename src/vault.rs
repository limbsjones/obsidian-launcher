use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct Note {
    pub path: PathBuf,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub wikilinks: Vec<String>,
}

pub struct Vault {
    pub root: PathBuf,
}

impl Vault {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn scan(&self) -> Result<Vec<Note>> {
        info!("Scanning vault at {:?}", self.root);
        let mut notes = Vec::new();
        self.scan_dir(&self.root, &mut notes)?;
        info!("Found {} notes", notes.len());
        Ok(notes)
    }

    fn scan_dir(&self, dir: &Path, notes: &mut Vec<Note>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if path.file_name().map_or(false, |n| {
                    n == ".git" || n == ".obsidian" || n == ".opencode" || n == "node_modules"
                }) {
                    continue;
                }
                self.scan_dir(&path, notes)?;
            } else if path.extension().map_or(false, |ext| ext == "md") {
                match self.parse_note(&path) {
                    Ok(note) => notes.push(note),
                    Err(e) => warn!("Failed to parse {:?}: {}", path, e),
                }
            }
        }
        Ok(())
    }

    fn parse_wikilinks(&self, content: &str) -> Vec<String> {
        let mut wikilinks = Vec::new();
        let mut remaining = content;
        while let Some(start) = remaining.find("[[") {
            if let Some(end) = remaining[start..].find("]") {
                let link_start = start + 2;
                let link_end = start + end;
                let link = &remaining[link_start..link_end];
                // Handle [[Title|Display]] — take the first part (the target)
                let target = link.split('|').next().unwrap_or("").trim();
                if !target.is_empty() {
                    wikilinks.push(target.to_string());
                }
                remaining = &remaining[link_end + 2..];
            } else {
                break;
            }
        }
        wikilinks
    }

    fn parse_note(&self, path: &Path) -> Result<Note> {
        let content = fs::read_to_string(path)?;
        let mut title = String::new();
        let mut text_content = String::new();
        let mut tags = Vec::new();

        let parser = Parser::new(&content);
        let mut in_h1 = false;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) if level == HeadingLevel::H1 => {
                    in_h1 = true;
                }
                Event::End(TagEnd::Heading(_)) if in_h1 => {
                    in_h1 = false;
                }
                Event::Text(text) => {
                    if in_h1 && title.is_empty() {
                        title = text.to_string();
                    }
                    text_content.push_str(&text);
                    text_content.push(' ');

                    for word in text.split_whitespace() {
                        if word.starts_with('#') && word.len() > 1 {
                            let tag = word.trim_start_matches('#');
                            let tag = tag.trim_end_matches(|c: char| c.is_ascii_punctuation());
                            if !tag.is_empty() {
                                tags.push(tag.to_string());
                            }
                        }
                    }
                }
                Event::Code(text) => {
                    text_content.push_str(&text);
                    text_content.push(' ');
                }
                _ => {}
            }
        }

        if title.is_empty() {
            title = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
        }

        // Parse wikilinks from the raw markdown content
        let wikilinks = self.parse_wikilinks(&content);

        info!("Parsed note: title='{}', content_len={}, wikilinks={:?}", title, text_content.len(), wikilinks);

        Ok(Note {
            path: path.to_path_buf(),
            title,
            content: text_content,
            tags,
            wikilinks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("obsidian-test-vault-{}-{}", name, id))
    }

    #[test]
    fn test_parse_note_h1_title() {
        let dir = test_dir("h1_title");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test-note.md");
        std::fs::write(&path, "# My Title\n\nSome content here.").unwrap();

        let vault = Vault::new(dir.clone());
        let note = vault.parse_note(&path).unwrap();

        assert_eq!(note.title, "My Title");
        assert!(note.content.contains("My Title"));
        assert!(note.content.contains("Some content here"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_note_title_from_filename_when_no_h1() {
        let dir = test_dir("filename_title");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("NoTitleNote.md");
        std::fs::write(&path, "Just some content.\n\nNo heading here.").unwrap();

        let vault = Vault::new(dir.clone());
        let note = vault.parse_note(&path).unwrap();

        assert_eq!(note.title, "NoTitleNote");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_note_extracts_wikilinks() {
        let dir = test_dir("wikilinks");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("wikilinks-note.md");
        std::fs::write(
            &path,
            "# Wikilinks Note\n\nThis references [[Another Note]] and [[Target Page|Display Text]] here.",
        )
        .unwrap();

        let vault = Vault::new(dir.clone());
        let note = vault.parse_note(&path).unwrap();

        assert!(note.wikilinks.contains(&"Another Note".to_string()));
        assert!(note.wikilinks.contains(&"Target Page".to_string()));
        assert_eq!(note.wikilinks.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_note_extracts_tags() {
        let dir = test_dir("tags");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tags-note.md");
        std::fs::write(&path, "# Tags Note\n\nThis is about #rust and #programming.").unwrap();

        let vault = Vault::new(dir.clone());
        let note = vault.parse_note(&path).unwrap();

        assert!(note.tags.contains(&"rust".to_string()));
        assert!(note.tags.contains(&"programming".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_skips_dot_git() {
        let dir = test_dir("scan_skips_git");
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::write(dir.join("note.md"), "# Hello").unwrap();
        std::fs::write(dir.join(".git").join("config"), "irrelevant").unwrap();

        let vault = Vault::new(dir.clone());
        let notes = vault.scan().unwrap();

        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "Hello");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
