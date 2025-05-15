// crbrs-lsp/src/main.rs
use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    notification::{DidChangeTextDocument, DidOpenTextDocument, DidSaveTextDocument, PublishDiagnostics},
    request::Shutdown,
    ClientCapabilities, Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, InitializeParams, InitializeResult,
    Position, PublishDiagnosticsParams, Range, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri,
};
use crbrs_lib::{Settings, CompilationErrorDetail}; // Import from your lib
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex}; // For sharing state like settings

// Simple struct to hold document state if needed
struct DocumentState {
    uri: Uri,
    content: String,
    version: Option<i32>,
}

fn main() -> anyhow::Result<()> {
    // Note: LSP server communicates over stdio by default.
    eprintln!("Starting crbrs-lsp server..."); // Log to stderr for debugging LSP startup

    // Create the transport. Includes the stdio (stdin and stdout) versions of the channel.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the main thread to end (typically by trigger LSP Exit event).
    let server_capabilities = initialize_server_capabilities();
    let initialize_params = connection.initialize(serde_json::to_value(&server_capabilities)?)?;
    let params: InitializeParams = serde_json::from_value(initialize_params)?;
    let _client_capabilities: ClientCapabilities = params.capabilities;
    // params.root_uri can be useful

    eprintln!("crbrs-lsp server initialized.");

    // Load crbrs settings (might need to be shareable if requests are handled in threads)
    let settings = Arc::new(Mutex::new(crbrs_lib::config::load_settings().unwrap_or_default()));
    // Store open documents
    let open_documents = Arc::new(Mutex::new(HashMap::<Uri, DocumentState>::new()));

    main_loop(connection, settings, open_documents)?;
    io_threads.join()?;

    eprintln!("crbrs-lsp server shutting down.");
    Ok(())
}

fn initialize_server_capabilities() -> ServerCapabilities {
    let mut capabilities = ServerCapabilities::default();
    // For "compile on save" or "compile on change":
    capabilities.text_document_sync = Some(TextDocumentSyncCapability::Kind(
        TextDocumentSyncKind::FULL, // Or INCREMENTAL if you handle deltas
    ));
    // Add other capabilities later: hover, completion, etc.
    capabilities
}

fn main_loop(
    connection: Connection,
    settings: Arc<Mutex<Settings>>,
    open_documents: Arc<Mutex<HashMap<Uri, DocumentState>>>,
) -> anyhow::Result<()> {
    for msg in &connection.receiver {
        eprintln!("LSP Received: {:?}", msg); // Debugging
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                // Handle other requests if needed
                // e.g., req.method == "textDocument/hover"
                eprintln!("LSP Unhandled request: {:?}", req);
                // Respond with an error for unhandled requests
                let resp = Response::new_err(req.id, lsp_server::ErrorCode::MethodNotFound as i32, "Method not found".to_string());
                connection.sender.send(Message::Response(resp))?;
            }
            Message::Response(resp) => {
                eprintln!("LSP Got response: {:?}", resp);
            }
            Message::Notification(not) => {
                match not.method.as_str() {
                    "textDocument/didOpen" => {
                        let params: DidOpenTextDocumentParams = serde_json::from_value(not.params)?;
                        let uri = params.text_document.uri;
                        let content = params.text_document.text;
                        let version = Some(params.text_document.version);
                        eprintln!("LSP Opened: {:?}", uri);
                        open_documents.lock().unwrap().insert(uri.clone(), DocumentState { uri: uri.clone(), content: content.clone(), version });
                        publish_diagnostics_for_uri(&connection, uri, &content, settings.lock().unwrap().clone())?;
                    }
                    "textDocument/didChange" => {
                        let params: DidChangeTextDocumentParams = serde_json::from_value(not.params)?;
                        let uri = params.text_document.uri;
                        // Assuming FULL sync, so contentChanges[0] has the full new text
                        if let Some(change) = params.content_changes.into_iter().next() {
                            let content = change.text;
                            let version = params.text_document.version;
                            eprint!("LSP Changed: {:?}", uri);
                            open_documents.lock().unwrap().insert(uri.clone(), DocumentState { uri: uri.clone(), content: content.clone(), version });
                            publish_diagnostics_for_uri(&connection, uri, &content, settings.lock().unwrap().clone())?;
                        }
                    }
                    "textDocument/didSave" => {
                        let params: DidSaveTextDocumentParams = serde_json::from_value(not.params)?;
                        let uri = params.text_document.uri;
                        eprintln!("LSP Saved: {:?}", uri);
                        // Re-trigger diagnostics on save, using the stored content
                        if let Some(doc_state) = open_documents.lock().unwrap().get(&uri) {
                            publish_diagnostics_for_uri(&connection, uri.clone(), &doc_state.content, settings.lock().unwrap().clone())?;
                        }
                    }
                    "exit" => return Ok(()),
                    _ => {
                        eprintln!("LSP Unhandled notification: {:?}", not.method);
                    }
                }
            }
        }
    }
    Ok(())
}

fn publish_diagnostics_for_uri(
    connection: &Connection,
    uri: Uri,
    content: &str, // Current content of the document
    settings: Settings, // Pass a clone of settings
) -> anyhow::Result<()> {
    eprintln!("LSP Publishing diagnostics for: {:?}", uri);
    let diagnostics = generate_diagnostics(uri.clone(), content, &settings);
    connection.sender.send(Message::Notification(Notification {
        method: PublishDiagnostics::METHOD.to_string(),
        params: serde_json::to_value(PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None, // Can set if tracking versions
        })?,
    }))?;
    Ok(())
}

fn generate_diagnostics(uri: Uri, content: &str, settings: &Settings) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let file_path = match uri.to_file_path() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("LSP Error: Could not convert URI to file path: {}", uri);
            return diagnostics; // Cannot compile if not a file path
        }
    };

    // Option 1: Save content to a temporary file to pass to compiler
    // This is often necessary if the compiler can only operate on files.
    let temp_dir = match tempfile::Builder::new().prefix("crbrs_lsp_").tempdir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("LSP Error: Could not create temp dir: {}", e);
            return diagnostics;
        }
    };
    let temp_file_path = temp_dir.path().join(file_path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("temp.crb")));
    if let Err(e) = std::fs::write(&temp_file_path, content) {
        eprintln!("LSP Error: Could not write to temp file {:?}: {}", temp_file_path, e);
        return diagnostics;
    }

    eprintln!("LSP Compiling temp file: {:?}", temp_file_path);

    // Reuse your library's compile function.
    // It needs to operate on the temp_file_path.
    // The `output_log_param` for `compile_file_impl` will be None by default,
    // so it will parse stdout.
    match crbrs_lib::compiler::compile_file_impl(&temp_file_path, None, None, settings) {
        Ok(_) => {
            eprintln!("LSP Compilation of {:?} successful (for diagnostics).", temp_file_path);
            // Clear existing diagnostics for this file if successful
        }
        Err(crbrs_lib::Error::CompilationFailed { errors, raw_log: _, .. }) => {
            eprintln!("LSP Compilation of {:?} failed (for diagnostics). {} errors found.", temp_file_path, errors.len());
            for err_detail in errors {
                let line = err_detail.line.unwrap_or(1).saturating_sub(1); // LSP lines are 0-indexed
                // Try to get actual line content to determine character range (can be complex)
                // For now, just highlight the whole line or a default range.
                let range = Range {
                    start: Position { line, character: 0 },
                    end: Position { line, character: u32::MAX }, // Highlight whole line
                };
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None, // Can add error codes if compiler provides them
                    code_description: None,
                    source: Some("crbrs".to_string()),
                    message: err_detail.message,
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }
        }
        Err(e) => {
            // Other types of errors from compile_file_impl (e.g., WineNotFound, IoError)
            eprintln!("LSP Error during background compilation for {:?}: {}", temp_file_path, e);
            // Optionally, create a general diagnostic for this file.
            diagnostics.push(Diagnostic {
                range: Range::default(), // Position 0,0
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("crbrs-lsp".to_string()),
                message: format!("crbrs tool error: {}", e),
                ..Default::default()
            });
        }
    }
    diagnostics
}