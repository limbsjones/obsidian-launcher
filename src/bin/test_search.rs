use obsidian_launcher::config::Config;
use obsidian_launcher::index::SearchIndex;
use obsidian_launcher::vault::Vault;

fn main() {
    let config = Config::load().expect("Failed to load config");

    println!("Vault path: {:?}", config.vault_path);
    println!("Index path: {:?}", config.index_path());

    let vault = Vault::new(config.vault_path.clone());
    let notes = vault.scan().expect("Failed to scan vault");

    println!("\nFound {} notes:", notes.len());
    for note in &notes {
        println!("  Title: '{}'", note.title);
        println!("  Content: '{}'", note.content);
        println!("  Tags: {:?}", note.tags);
        println!();
    }

    let mut search_index = SearchIndex::open_or_create(&config.index_path()).expect("Failed to open index");
    search_index.index_notes(&notes).expect("Failed to index notes");

    println!("\n--- Search tests ---");

    for query in &["youtube", "structure", "man", "pages", "program", "search", "yout", "stru", "sea", "prog", "t", "te", "man p"] {
        let results = search_index.search(query, config.max_results).expect("Search failed");
        println!("Query '{}': {} results", query, results.len());
        for (title, path, wikilinks) in &results {
            let wl = if wikilinks.is_empty() { String::new() } else { format!(" [[{}]]", wikilinks.len()) };
            println!("  -> {} ({}){}", title, path, wl);
        }
    }
}
