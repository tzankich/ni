use tower_lsp::{LspService, Server};

mod analysis;
mod backend;
mod completion;
mod definition;
mod document;
mod hover;
mod symbols;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(backend::NiLanguageServer::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
