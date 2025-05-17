// crbrs-lsp/src/main.rs
use lsp_server::{Connection, Message, Notification as LspServerNotification, Response}; // Renamed to avoid conflict
use lsp_types::{
    notification::{PublishDiagnostics, Notification as LspNotificationTrait}, // LspNotificationTrait for ::METHOD
    ClientCapabilities, Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, InitializeParams,
    Position, PublishDiagnosticsParams, Range, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri, // This is lsp_types::Uri
};
use crbrs_lib::{Settings, Error as CrbrsError, CompilationErrorDetail}; // Import your lib's Error and other types
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// Simple struct to hold document state
struct DocumentState {
    uri: Uri,
    content: String,
    version: Option<i32>, // LSP versions are optional integers
}

// Helper function to convert a file URI to a PathBuf
fn file_uri_to_pathbuf(uri: &Uri) -> Result<PathBuf, String> {
    if uri.scheme().map_or(false, |s| s.as_str() == "file") {
        let path_str = uri.path().as_str(); // from fluent_uri::Path

        #[cfg(windows)]
        let corrected_path_str = if path_str.starts_with('/') && path_str.get(1..3).map_or(false, |s| s.chars().nth(1) == Some(':')) {
            path_str.get(1..).unwrap_or(path_str)
        } else {
            path_str
        };
        #[cfg(not(windows))]
        let corrected_path_str = path_str;

        // fluent_uri's path().as_str() should provide a decoded path for file URIs.
        // If issues arise with specific characters (e.g. spaces), explicit percent-decoding might be needed here.
        Ok(PathBuf::from(corrected_path_str))
    } else {
        Err(format!("URI scheme is not 'file': {:?}", uri.scheme().map_or("<none>", |s| s.as_str())))
    }
}


fn main() -> anyhow::Result<()> {
    eprintln!("Starting crbrs-lsp server...");

    let (connection, io_threads) = Connection::stdio();
    let server_capabilities = initialize_server_capabilities();

    // Initialize the connection
    let initialize_params_json = connection.initialize(serde_json::to_value(&server_capabilities)?)?;
    let initialize_params: InitializeParams = serde_json::from_value(initialize_params_json)?;
    let _client_capabilities: ClientCapabilities = initialize_params.capabilities;
    // initialize_params.root_uri could be useful for workspace settings

    eprintln!("crbrs-lsp server initialized.");

    let settings = Arc::new(Mutex::new(
        crbrs_lib::config::load_settings().unwrap_or_else(|e| {
            eprintln!("LSP: Failed to load crbrs settings: {}. Using defaults.", e);
            Settings::default()
        }),
    ));
    let open_documents = Arc::new(Mutex::new(HashMap::<Uri, DocumentState>::new()));

    main_loop(connection, settings, open_documents)?;
    io_threads.join()?;

    eprintln!("crbrs-lsp server shutting down.");
    Ok(())
}

fn initialize_server_capabilities() -> ServerCapabilities {
    let mut capabilities = ServerCapabilities::default();
    capabilities.text_document_sync = Some(TextDocumentSyncCapability::Kind(
        TextDocumentSyncKind::FULL,
    ));
    // Add other capabilities as you implement them (hover, completion, etc.)
    capabilities
}

fn main_loop(
    connection: Connection,
    settings: Arc<Mutex<Settings>>,
    open_documents: Arc<Mutex<HashMap<Uri, DocumentState>>>,
) -> anyhow::Result<()> {
    for msg in &connection.receiver {
        eprintln!("LSP Received: type = {:?}", msg.type_name()); // More concise logging
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    eprintln!("LSP: Shutdown request received, exiting main loop.");
                    return Ok(());
                }
                // Handle other requests (hover, completion) here in the future
                eprintln!("LSP: Unhandled request: method = {}", req.method);
                let resp = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::MethodNotFound as i32,
                    format!("Method '{}' not handled.", req.method),
                );
                connection.sender.send(Message::Response(resp))?;
            }
            Message::Response(resp) => {
                eprintln!("LSP: Received unexpected response: id = {:?}", resp.id);
            }
            Message::Notification(not) => {
                match not.method.as_str() {
                    "textDocument/didOpen" => {
                        let params: DidOpenTextDocumentParams = serde_json::from_value(not.params)?;
                        let doc = params.text_document;
                        eprintln!("LSP: Opened file: {:?}", doc.uri);
                        let document_state = DocumentState {
                            uri: doc.uri.clone(),
                            content: doc.text.clone(),
                            version: Some(doc.version),
                        };
                        open_documents.lock().unwrap().insert(doc.uri.clone(), document_state);
                        // It's good practice to clone settings for each diagnostic run if they might change,
                        // or ensure settings are read fresh when needed.
                        let current_settings = settings.lock().unwrap().clone();
                        publish_diagnostics_for_uri(&connection, doc.uri, &doc.text, ¤t_settings)?;
                    }
                    "textDocument/didChange" => {
                        let params: DidChangeTextDocumentParams = serde_json::from_value(not.params)?;
                        let doc_id = params.text_document;
                        // Assuming FULL sync, contentChanges[0] has the full new text
                        if let Some(change) = params.content_changes.into_iter().next() {
                            eprintln!("LSP: Changed file: {:?}", doc_id.uri);
                            let document_state = DocumentState {
                                uri: doc_id.uri.clone(),
                                content: change.text.clone(),
                                version: Some(doc_id.version),
                            };
                            open_documents.lock().unwrap().insert(doc_id.uri.clone(), document_state);
                            let current_settings = settings.lock().unwrap().clone();
                            publish_diagnostics_for_uri(&connection, doc_id.uri, &change.text, ¤t_settings)?;
                        }
                    }
                    "textDocument/didSave" => {
                        let params: DidSaveTextDocumentParams = serde_json::from_value(not.params)?;
                        let doc_id = params.text_document;
                        eprintln!("LSP: Saved file: {:?}", doc_id.uri);
                        // Re-trigger diagnostics on save, using the stored content
                        if let Some(doc_state) = open_documents.lock().unwrap().get(&doc_id.uri) {
                            let current_settings = settings.lock().unwrap().clone();
                            publish_diagnostics_for_uri(&connection, doc_id.uri.clone(), &doc_state.content, ¤t_settings)?;
                        } else {
                            eprintln!("LSP Warning: didSave received for unknown document: {:?}", doc_id.uri);
                        }
                    }
                    "exit" => {
                        eprintln!("LSP: Exit notification received.");
                        return Ok(()); // Exit main loop
                    }
                    _ => {
                        eprintln!("LSP: Unhandled notification: method = {}", not.method);
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
    content: &str,
    settings: &Settings, // Pass settings by reference
) -> anyhow::Result<()> {
    eprintln!("LSP: Publishing diagnostics for: {:?}", uri);
    let diagnostics = generate_diagnostics(uri.clone(), content, settings);
    connection.sender.send(Message::Notification(LspServerNotification { // Use lsp_server::Notification
        method: lsp_types::notification::PublishDiagnostics::METHOD.to_string(),
        params: serde_json::to_value(PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None, // Can set if tracking versions from DocumentState
        })?,
    }))?;
    Ok(())
}

fn generate_diagnostics(uri: Uri, content: &str, settings: &Settings) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let file_path = match file_uri_to_pathbuf(&uri) {
        Ok(p) => p,
        Err(err_msg) => {
            eprintln!("LSP Error: Could not convert URI to file path: {} (URI: {:?})", err_msg, uri);
            diagnostics.push(Diagnostic {
                range: Range::default(), severity: Some(DiagnosticSeverity::ERROR),
                source: Some("crbrs-lsp".to_string()),
                message: format!("Invalid document URI for compilation: {}. URI: {:?}", err_msg, uri),
                ..Default::default()
            });
            return diagnostics;
        }
    };

    let temp_dir = match tempfile::Builder::new().prefix("crbrs_lsp_").tempdir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("LSP Error: Could not create temp dir: {}", e);
            diagnostics.push(Diagnostic {
                range: Range::default(), severity: Some(DiagnosticSeverity::ERROR),
                source: Some("crbrs-lsp".to_string()),
                message: "Internal LSP error: Could not create temporary directory.".to_string(),
                ..Default::default()
            });
            return diagnostics;
        }
    };

    let original_filename = file_path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("lsp_temp.crb"));
    let temp_file_path = temp_dir.path().join(original_filename);

    if let Err(e) = std::fs::write(&temp_file_path, content) {
        eprintln!("LSP Error: Could not write to temp file {:?}: {}", temp_file_path, e);
        diagnostics.push(Diagnostic {
            range: Range::default(), severity: Some(DiagnosticSeverity::ERROR),
            source: Some("crbrs-lsp".to_string()),
            message: "Internal LSP error: Could not write temporary file for compilation.".to_string(),
            ..Default::default()
        });
        return diagnostics;
    }

    eprintln!("LSP: Compiling temp file for diagnostics: {:?}", temp_file_path);

    match crbrs_lib::compiler::compile_file_impl(&temp_file_path, None, None, settings) {
        Ok(_) => {
            eprintln!("LSP: Background compilation successful for {:?}.", temp_file_path);
        }
        Err(CrbrsError::CompilationFailed { errors, .. }) => { // Use CrbrsError enum
            eprintln!("LSP: Background compilation of {:?} failed. {} errors found.", temp_file_path, errors.len());
            for err_detail in errors {
                let line_0_indexed = err_detail.line.unwrap_or(1).saturating_sub(1);
                let range = Range {
                    start: Position { line: line_0_indexed, character: 0 },
                    end: Position { line: line_0_indexed, character: u32::MAX },
                };
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("crbrs-compiler".to_string()),
                    message: err_detail.message,
                    ..Default::default()
                });
            }
        }
        Err(other_crbrs_error) => { // Catch other crbrs_lib::Error variants
            eprintln!("LSP: Error during background compilation for {:?}: {}", temp_file_path, other_crbrs_error);
            diagnostics.push(Diagnostic {
                range: Range::default(),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("crbrs-lsp".to_string()),
                message: format!("crbrs tool error during compilation: {}", other_crbrs_error),
                ..Default::default()
            });
        }
    }
    diagnostics
}