import * as path from 'path';
import * as vscode from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const serverOptions: ServerOptions = {
    command: 'ilo',
    args: ['lsp'],
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'ilo' }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.ilo'),
    },
  };

  client = new LanguageClient(
    'ilo',
    'ilo Language Server',
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
