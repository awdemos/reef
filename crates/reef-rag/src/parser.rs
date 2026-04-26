use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::RagResult;

/// A plugin that transforms raw file bytes into structured document blocks.
#[async_trait]
pub trait ParserPlugin: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn supported_mime_types(&self) -> Vec<&str>;

    async fn parse(&self, filename: &str, bytes: &[u8]) -> RagResult<ParseResult>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParseResult {
    pub document: ParsedDocument,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedDocument {
    pub title: Option<String>,
    pub blocks: Vec<DocumentBlock>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentBlock {
    Text {
        content: String,
        page_number: Option<usize>,
    },
    Table {
        rows: Vec<Vec<String>>,
        caption: Option<String>,
        page_number: Option<usize>,
    },
    Image {
        alt_text: Option<String>,
        caption: Option<String>,
        page_number: Option<usize>,
    },
    Heading {
        level: u8,
        content: String,
        page_number: Option<usize>,
    },
}

/// A lightweight markdown/text parser as the default.
pub struct DefaultParser;

#[async_trait]
impl ParserPlugin for DefaultParser {
    fn name(&self) -> &str {
        "default"
    }

    fn supported_mime_types(&self) -> Vec<&str> {
        vec![
            "text/plain",
            "text/markdown",
            "text/x-markdown",
            "application/json",
        ]
    }

    async fn parse(&self, _filename: &str, bytes: &[u8]) -> RagResult<ParseResult> {
        let content = String::from_utf8_lossy(bytes);
        let blocks = vec![DocumentBlock::Text {
            content: content.to_string(),
            page_number: None,
        }];

        Ok(ParseResult {
            document: ParsedDocument {
                title: None,
                blocks,
                metadata: serde_json::Value::Null,
            },
        })
    }
}
