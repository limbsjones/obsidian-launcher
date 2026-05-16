use std::path::Path;

use anyhow::Result;
use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{IndexRecordOption, Schema, STORED, TextFieldIndexing, TextOptions, Value},
    tokenizer::{LowerCaser, NgramTokenizer, TextAnalyzer},
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
    fn build_schema() -> Schema {
        let ngram_indexing = TextFieldIndexing::default()
            .set_tokenizer("ngram")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);

        let mut title_options = TextOptions::default();
        title_options = title_options.set_indexing_options(ngram_indexing.clone());
        title_options = title_options.set_stored();

        let mut content_options = TextOptions::default();
        content_options = content_options.set_indexing_options(ngram_indexing.clone());

        let mut tags_options = TextOptions::default();
        tags_options = tags_options.set_indexing_options(ngram_indexing);

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("title", title_options);
        schema_builder.add_text_field("content", content_options);
        schema_builder.add_text_field("path", STORED);
        schema_builder.add_text_field("tags", tags_options);
        schema_builder.build()
    }

    fn register_ngram_tokenizer(index: &mut Index) -> Result<()> {
        let ngram_tokenizer = NgramTokenizer::new(NGRAM_MIN, NGRAM_MAX, false)
            .map_err(|e| anyhow::anyhow!("Failed to create ngram tokenizer: {}", e))?;
        let ngram_analyzer = TextAnalyzer::builder(ngram_tokenizer)
            .filter(LowerCaser)
            .build();
        index.tokenizers().register("ngram", ngram_analyzer);
        Ok(())
    }

    pub fn new(index_path: &Path) -> Result<Self> {
        let schema = Self::build_schema();

        std::fs::create_dir_all(index_path)?;

        let mut index = Index::create_in_dir(index_path, schema.clone())?;
        Self::register_ngram_tokenizer(&mut index)?;

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

        Self::register_ngram_tokenizer(&mut index)?;

        Ok(Self { schema, index })
    }

    pub fn index_notes(&mut self, notes: &[Note]) -> Result<()> {
        info!("Indexing {} notes...", notes.len());

        let title = self.schema.get_field("title").expect("schema field 'title'");
        let content = self.schema.get_field("content").expect("schema field 'content'");
        let path = self.schema.get_field("path").expect("schema field 'path'");
        let tags = self.schema.get_field("tags").expect("schema field 'tags'");

        let mut index_writer: IndexWriter = self.index.writer(50_000_000)?;

        index_writer.delete_all_documents()?;

        for note in notes {
            let path_str = note.path.to_string_lossy().to_string();
            let tags_str = note.tags.join(" ");

            index_writer.add_document(doc![
                title => note.title.as_str(),
                content => note.content.as_str(),
                path => path_str.as_str(),
                tags => tags_str.as_str(),
            ])?;
        }

        index_writer.commit()?;
        info!("Indexing complete");
        Ok(())
    }

    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<(String, String)>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let title = self.schema.get_field("title").expect("schema field 'title'");
        let content = self.schema.get_field("content").expect("schema field 'content'");
        let path = self.schema.get_field("path").expect("schema field 'path'");
        let tags = self.schema.get_field("tags").expect("schema field 'tags'");

        let query_parser = QueryParser::for_index(&self.index, vec![title, content, tags]);
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

        let top_docs = TopDocs::with_limit(limit);
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
            info!("Hit: title='{}', path='{}'", title_val, path_val);
            hits.push((title_val, path_val));
        }

        Ok(hits)
    }
}
