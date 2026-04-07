import * as path from "path";
import { workspace, ExtensionContext } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: ExtensionContext) {
  // Find the ni_lsp binary — check bundled location first, then $PATH
  const bundled = context.asAbsolutePath(path.join("bin", "ni_lsp"));
  const command = require("fs").existsSync(bundled) ? bundled : "ni_lsp";

  const serverOptions: ServerOptions = {
    run: { command, transport: TransportKind.stdio },
    debug: { command, transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "ni" }],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/*.ni"),
    },
  };

  client = new LanguageClient(
    "niLanguageServer",
    "Ni Language Server",
    serverOptions,
    clientOptions
  );

  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
