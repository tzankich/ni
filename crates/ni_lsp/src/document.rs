use std::collections::HashMap;
use tower_lsp::lsp_types::*;

use ni_parser::Program;

const MAX_DOCUMENT_SIZE: usize = 10 * 1024 * 1024; // 10 MB
const MAX_DOCUMENT_COUNT: usize = 1000;

pub struct DocumentState {
    pub source: String,
    pub program: Option<Program>,
    pub version: i32,
}

pub struct DocumentStore {
    documents: HashMap<Url, DocumentState>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn open(&mut self, uri: Url, text: String, version: i32) {
        if text.len() > MAX_DOCUMENT_SIZE {
            return;
        }
        if self.documents.len() >= MAX_DOCUMENT_COUNT {
            return;
        }
        self.documents.insert(
            uri,
            DocumentState {
                source: text,
                program: None,
                version,
            },
        );
    }

    pub fn update(&mut self, uri: &Url, text: String, version: i32) {
        if text.len() > MAX_DOCUMENT_SIZE {
            return;
        }
        if let Some(doc) = self.documents.get_mut(uri) {
            doc.source = text;
            doc.version = version;
            doc.program = None;
        }
    }

    pub fn close(&mut self, uri: &Url) {
        self.documents.remove(uri);
    }

    pub fn get(&self, uri: &Url) -> Option<&DocumentState> {
        self.documents.get(uri)
    }

    pub fn set_program(&mut self, uri: &Url, program: Program) {
        if let Some(doc) = self.documents.get_mut(uri) {
            doc.program = Some(program);
        }
    }
}
