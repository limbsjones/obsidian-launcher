use std::path::Path;

use anyhow::{Context, Result};
use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{IndexRecordOption, Schema, STORED, TextFieldIndexing, TextOptions, Value},
    tokenizer::{Language, LowerCaser, NgramTokenizer, Stemmer, TextAnalyzer},
    Index, IndexWriter, TantivyDocument,
};
use tracing::info;

use crate::vault::Note;

const NGRAM_MIN: usize = 1;
const NGRAM_MAX: usize = 15;

pub struct SearchIndex {
    schema: Schema,
    index: Index,
}

impl SearchIndex {
    /// Build the schema with two complementary tokenization strategies:
    ///
    /// - **ngram** (`title`, `content`, `tags`, `wikilinks`):
    ///   splits text into overlapping substrings (`NgramTokenizer`)
    ///   for fuzzy / prefix / substring matching.
    ///
    /// - **french** (`title_stem`, `content_stem`):
    ///   tokenises with `SimpleTokenizer` + `LowerCaser`
    ///   + `Stemmer(Language::French)` for French morphology
    ///   (plural → singular, verb conjugation, etc.).
    ///
    /// The query parser searches both sets of fields, so a search
    /// for *"rechercher"* matches notes containing *"recherche"*
    /// (via the stemmer) AND notes containing *"échrcher"*
    /// (typo tolerance via ngram).
    fn build_schema() -> Schema {
        let ngram_indexing = TextFieldIndexing::default()
            .set_tokenizer("ngram")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);

        let french_indexing = TextFieldIndexing::default()
            .set_tokenizer("fr")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);

        // ---- ngram fields ----
        let mut title_ngram = TextOptions::default();
        title_ngram = title_ngram.set_indexing_options(ngram_indexing.clone());
        title_ngram = title_ngram.set_stored();

        let mut content_ngram = TextOptions::default();
        content_ngram = content_ngram.set_indexing_options(ngram_indexing.clone());

        let mut tags_ngram = TextOptions::default();
        tags_ngram = tags_ngram.set_indexing_options(ngram_indexing.clone());

        let mut wikilinks_ngram = TextOptions::default();
        wikilinks_ngram = wikilinks_ngram.set_indexing_options(ngram_indexing.clone());

        // ---- French-stemmed fields ----
        let mut title_fr = TextOptions::default();
        title_fr = title_fr.set_indexing_options(french_indexing.clone());
        // Also store the title so we can read it back
        title_fr = title_fr.set_stored();

        let mut content_fr = TextOptions::default();
        content_fr = content_fr.set_indexing_options(french_indexing);
        // Path is only stored, not indexed for search

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("title", title_ngram);      // ngram + stored
        schema_builder.add_text_field("title_stem", title_fr);    // french stemmer + stored
        schema_builder.add_text_field("content", content_ngram);  // ngram
        schema_builder.add_text_field("content_stem", content_fr); // french stemmer
        schema_builder.add_text_field("path", STORED);
        schema_builder.add_text_field("tags", tags_ngram);        // ngram
        schema_builder.add_text_field("wikilinks", wikilinks_ngram); // ngram
        schema_builder.build()
    }

    fn register_tokenizers(index: &mut Index) -> Result<()> {
        // 1. Ngram tokenizer for fuzzy/prefix matching
        let ngram_tokenizer = NgramTokenizer::new(NGRAM_MIN, NGRAM_MAX, false)
            .map_err(|e| anyhow::anyhow!("Failed to create ngram tokenizer: {}", e))?;
        let ngram_analyzer = TextAnalyzer::builder(ngram_tokenizer)
            .filter(LowerCaser)
            .build();
        index.tokenizers().register("ngram", ngram_analyzer);

        // 2. French tokenizer: simple tokenisation + lowercasing + stemming
        let french_analyzer = TextAnalyzer::builder(
            tantivy::tokenizer::SimpleTokenizer::default()
        )
        .filter(LowerCaser)
        .filter(Stemmer::new(Language::French))
        .build();
        index.tokenizers().register("fr", french_analyzer);

        Ok(())
    }

    pub fn new(index_path: &Path) -> Result<Self> {
        let schema = Self::build_schema();

        std::fs::create_dir_all(index_path)?;

        let mut index = Index::create_in_dir(index_path, schema.clone())?;
        Self::register_tokenizers(&mut index)?;

        Ok(Self { schema, index })
    }

    pub fn open_or_create(index_path: &Path) -> Result<Self> {
        let schema = Self::build_schema();

        std::fs::create_dir_all(index_path)?;

        let mut index = if Index::open_in_dir(index_path).is_ok() {
            Index::open_in_dir(index_path)?
        } else {
            Index::create_in_dir(index_path, schema.clone())?
        };

        Self::register_tokenizers(&mut index)?;

        Ok(Self { schema, index })
    }

    pub fn index_notes(&mut self, notes: &[Note]) -> Result<()> {
        info!("Indexing {} notes...", notes.len());

        let title = self.get_field("title")?;
        let title_stem = self.get_field("title_stem")?;
        let content = self.get_field("content")?;
        let content_stem = self.get_field("content_stem")?;
        let path = self.get_field("path")?;
        let tags = self.get_field("tags")?;
        let wikilinks = self.get_field("wikilinks")?;

        let mut index_writer: IndexWriter<TantivyDocument> = self.index.writer(50_000_000)?;

        index_writer.delete_all_documents()?;

        for note in notes {
            let path_str = note.path.to_string_lossy().to_string();
            let tags_str = note.tags.join(" ");
            let wikilinks_str = note.wikilinks.join(" ");

            index_writer.add_document(doc![
                title => note.title.as_str(),
                title_stem => note.title.as_str(),
                content => note.content.as_str(),
                content_stem => note.content.as_str(),
                path => path_str.as_str(),
                tags => tags_str.as_str(),
                wikilinks => wikilinks_str.as_str(),
            ])?;
        }

        index_writer.commit()?;
        info!("Indexing complete");
        Ok(())
    }

    /// Get a schema field by name, returning an error instead of panicking.
    fn get_field(&self, name: &str) -> Result<tantivy::schema::Field> {
        self.schema
            .get_field(name)
            .with_context(|| format!("Schema field '{}' not found — index may be corrupt or from a different version", name))
    }

    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<(String, String, Vec<String>)>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let title = self.get_field("title")?;
        let title_stem = self.get_field("title_stem")?;
        let content = self.get_field("content")?;
        let content_stem = self.get_field("content_stem")?;
        let path = self.get_field("path")?;
        let tags = self.get_field("tags")?;
        let wikilinks = self.get_field("wikilinks")?;

        // Search across all indexed fields (ngram + French-stemmed)
        let query_parser = QueryParser::for_index(
            &self.index,
            vec![title, content, tags, wikilinks, title_stem, content_stem],
        );
        let escaped = query_str
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let query = match query_parser.parse_query(&format!("\"{}\"", escaped)) {
            Ok(q) => q,
            Err(e) => {
                info!("Query parse error for '{}': {}", escaped, e);
                return Ok(Vec::new());
            }
        };

        info!("Searching for '{}' with limit {}", query_str, limit);

        let top_docs = TopDocs::with_limit(limit).order_by_score();
        let results = searcher.search(&query, &top_docs)?;

        info!("Found {} raw results", results.len());

        let mut hits = Vec::new();
        for (_score, doc_address) in results {
            let doc: TantivyDocument = searcher.doc(doc_address)?;
            let title_val = doc
                .get_first(title)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let path_val = doc
                .get_first(path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let wikilinks_val = doc
                .get_first(wikilinks)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let wikilinks_list: Vec<String> = if wikilinks_val.is_empty() {
                Vec::new()
            } else {
                wikilinks_val.split_whitespace().map(|s| s.to_string()).collect()
            };
            info!("Hit: title='{}', path='{}'", title_val, path_val);
            hits.push((title_val, path_val, wikilinks_list));
        }

        Ok(hits)
    }
}
