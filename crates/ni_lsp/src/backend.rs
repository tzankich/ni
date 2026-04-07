use std::sync::Mutex;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::analysis;
use crate::completion;
use crate::definition;
use crate::document::DocumentStore;
use crate::hover;
use crate::symbols;

/// Convert UTF-16 code units (LSP Position.character) to a byte offset within a line.
fn utf16_to_byte_offset(line: &str, utf16_col: usize) -> usize {
    let mut utf16_count = 0;
    for (byte_idx, ch) in line.char_indices() {
        if utf16_count >= utf16_col {
            return byte_idx;
        }
        utf16_count += ch.len_utf16();
    }
    line.len()
}


fn span_to_range(span: ni_error::Span) -> Range {
    let line = span.line.saturating_sub(1) as u32;
    // Ni columns are char-based; for BMP this equals UTF-16 code units.
    let col = span.column.saturating_sub(1) as u32;
    let end_line = span.end_line.saturating_sub(1) as u32;
    let end_col = if span.end_column > 0 {
        (span.end_column - 1) as u32
    } else {
        col + 1
    };
    Range {
        start: Position::new(line, col),
        end: Position::new(end_line, end_col),
    }
}

pub struct NiLanguageServer {
    client: Client,
    documents: Mutex<DocumentStore>,
}

impl NiLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Mutex::new(DocumentStore::new()),
        }
    }

    async fn analyze_and_publish(&self, uri: Url, source: &str) {
        let result = analysis::analyze(source);

        if let Some(program) = result.program {
            let mut docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
            docs.set_program(&uri, program);
        }

        self.client
            .publish_diagnostics(uri, result.diagnostics, None)
            .await;
    }

    fn get_word_at_position(&self, uri: &Url, pos: Position) -> Option<String> {
        let docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        let doc = docs.get(uri)?;
        let line = doc.source.lines().nth(pos.line as usize)?;
        // LSP Position.character is UTF-16 code units, not bytes
        let col = utf16_to_byte_offset(line, pos.character as usize);
        if col > line.len() {
            return None;
        }

        // Find word boundaries (rfind/find return byte offsets)
        let start = line[..col]
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| {
                // Advance past the full character, not just one byte
                i + line[i..].chars().next().map_or(1, |c| c.len_utf8())
            })
            .unwrap_or(0);
        let end = line[col..]
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| i + col)
            .unwrap_or(line.len());

        if start >= end {
            return None;
        }

        Some(line[start..end].to_string())
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for NiLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "ni_lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Ni Language Server ready")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();
        let version = params.text_document.version;

        {
            let mut docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
            docs.open(uri.clone(), text.clone(), version);
        }

        self.analyze_and_publish(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let version = params.text_document.version;

        // We use full sync, so there's exactly one change with the full text
        if let Some(change) = params.content_changes.into_iter().next() {
            let text = change.text.clone();
            {
                let mut docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
                docs.update(&uri, text.clone(), version);
            }
            self.analyze_and_publish(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        docs.close(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        if let Some(word) = self.get_word_at_position(uri, pos) {
            Ok(hover::hover_for_word(&word))
        } else {
            Ok(None)
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        let doc = match docs.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let program = match &doc.program {
            Some(p) => p,
            None => return Ok(None),
        };

        // LSP positions are 0-indexed (UTF-16 code units); Ni spans are 1-indexed (char-based).
        // For BMP characters, UTF-16 code units == char count, so +1 suffices.
        let line = pos.line as usize + 1;
        let col = pos.character as usize + 1;

        let (name, _) = match definition::identifier_at_position(program, line, col) {
            Some(r) => r,
            None => return Ok(None),
        };

        let symbols = definition::SymbolTable::build(program);
        let scope = symbols.scope_at_position(line, col);
        let def = match symbols.find(&name, scope) {
            Some(d) => d,
            None => return Ok(None),
        };

        let range = span_to_range(def.span);
        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range,
        })))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut items = completion::keyword_completions();

        // Add contextual completions from AST
        let uri = &params.text_document_position.text_document.uri;
        {
            let docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(doc) = docs.get(uri) {
                if let Some(ref program) = doc.program {
                    items.extend(completion::identifiers_from_program(program));
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(doc) = docs.get(uri) {
            if let Some(ref program) = doc.program {
                let syms = symbols::document_symbols(program);
                return Ok(Some(DocumentSymbolResponse::Nested(syms)));
            }
        }
        Ok(None)
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = &params.text_document.uri;
        let docs = self.documents.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(doc) = docs.get(uri) {
            match ni_fmt::format(&doc.source) {
                Ok(formatted) => {
                    // Replace entire document; use u32::MAX so LSP clients
                    // clamp to the actual document end regardless of content.
                    Ok(Some(vec![TextEdit {
                        range: Range {
                            start: Position::new(0, 0),
                            end: Position::new(u32::MAX, u32::MAX),
                        },
                        new_text: formatted,
                    }]))
                }
                Err(_) => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
}
