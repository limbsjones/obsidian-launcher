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
    pub links: Vec<String>,
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
                if path.file_name().map_or(false, |n| n == ".git" || n == ".obsidian") {
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

    fn parse_note(&self, path: &Path) -> Result<Note> {
        let content = fs::read_to_string(path)?;
        let mut title = String::new();
        let mut text_content = String::new();
        let mut tags = Vec::new();
        let mut links = Vec::new();

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
                            tags.push(word.trim_start_matches('#').to_string());
                        }
                    }
                }
                Event::Code(text) => {
                    text_content.push_str(&text);
                    text_content.push(' ');
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    links.push(dest_url.to_string());
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

        info!("Parsed note: title='{}', content_len={}", title, text_content.len());

        Ok(Note {
            path: path.to_path_buf(),
            title,
            content: text_content,
            tags,
            links,
        })
    }
}
