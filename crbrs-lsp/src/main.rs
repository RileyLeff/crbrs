// crbrs-lsp/src/main.rs
use lsp_server::{Connection, Message, Notification as LspServerNotification, Response};
use lsp_types::{
    notification::{PublishDiagnostics, Notification as LspNotificationTrait},
    ClientCapabilities, Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, InitializeParams,
    Position, PublishDiagnosticsParams, Range, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri,
};
use crbrs_lib::{Settings, Error as CrbrsError, CompilationErrorDetail};
use std::collections::HashMap;
use std::path::PathBuf; // Keep this for file_uri_to_pathbuf
use std::sync::{Arc, Mutex};

struct DocumentState {
    uri: Uri,
    content: String,
    version: Option<i32>,
}

fn file_uri_to_pathbuf(uri: &Uri) -> Result<PathBuf, String> {
    if uri.scheme().map_or(false, |s| s.as_str() == "file") {
        let path_str = uri.path().as_str();
        #[cfg(windows)]
        let corrected_path_str = if path_str.starts_with('/') && path_str.get(1..3).map_or(false, |s| s.chars().nth(1) == Some(':')) {
            path_str.get(1..).unwrap_or(path_str)
        } else { path_str };
        #[cfg(not(windows))]
        let corrected_path_str = path_str;
        Ok(PathBuf::from(corrected_path_str))
    } else {
        Err(format!("URI scheme is not 'file': {:?}", uri.scheme().map_or("<none>", |s| s.as_str())))
    }
}

fn main() -> anyhow::Result<()> {
    eprintln!("Starting crbrs-lsp server...");
    let (connection, io_threads) = Connection::stdio();
    let server_capabilities = initialize_server_capabilities();
    let initialize_params_json = connection.initialize(serde_json::to_value(&server_capabilities)?)?;
    let initialize_params: InitializeParams = serde_json::from_value(initialize_params_json)?;
    let _client_capabilities: ClientCapabilities = initialize_params.capabilities;
    eprintln!("crbrs-lsp server initialized.");

    let settings_arc = Arc::new(Mutex::new(
        crbrs_lib::config::load_settings().unwrap_or_else(|e| {
            eprintln!("LSP: Failed to load crbrs settings: {}. Using defaults.", e);
            Settings::default()
        }),
    ));
    let open_documents_arc = Arc::new(Mutex::new(HashMap::<Uri, DocumentState>::new()));

    main_loop(connection, settings_arc, open_documents_arc)?;
    io_threads.join()?;
    eprintln!("crbrs-lsp server shutting down.");
    Ok(())
}

fn initialize_server_capabilities() -> ServerCapabilities {
    let mut capabilities = ServerCapabilities::default();
    capabilities.text_document_sync = Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL));
    capabilities
}

fn main_loop(
    connection: Connection,
    settings_arc: Arc<Mutex<Settings>>, // Renamed for clarity
    open_documents_arc: Arc<Mutex<HashMap<Uri, DocumentState>>>, // Renamed for clarity
) -> anyhow::Result<()> {
    for msg in &connection.receiver {
        // For concise logging, let's see the method for requests/notifications
        match &msg {
            Message::Request(req) => eprintln!("LSP Received Request: method = {}", req.method),
            Message::Notification(not) => eprintln!("LSP Received Notification: method = {}", not.method),
            Message::Response(_) => eprintln!("LSP Received Response"),
        }

        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    eprintln!("LSP: Shutdown request received, exiting main loop.");
                    return Ok(());
                }
                eprintln!("LSP: Unhandled request: method = {}", req.method);
                let resp = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::MethodNotFound as i32,
                    format!("Method '{}' not handled by crbrs-lsp.", req.method),
                );
                connection.sender.send(Message::Response(resp))?;
            }
            Message::Response(resp) => {
                eprintln!("LSP: Received (and ignored) response: id = {:?}", resp.id);
            }
            Message::Notification(not) => {
                match not.method.as_str() {
                    "textDocument/didOpen" => {
                        let params: DidOpenTextDocumentParams = serde_json::from_value(not.params)?;
                        let doc_text = params.text_document; // This is TextDocumentItem
                        eprintln!("LSP: Opened file: {:?}", doc_text.uri);
                        let document_state = DocumentState {
                            uri: doc_text.uri.clone(),
                            content: doc_text.text.clone(),
                            version: Some(doc_text.version),
                        };
                        open_documents_arc.lock().unwrap().insert(doc_text.uri.clone(), document_state);
                        let current_settings = settings_arc.lock().unwrap().clone(); // Clone settings for this task
                        publish_diagnostics_for_uri(&connection, doc_text.uri, &doc_text.text, &current_settings)?;
                    }
                    "textDocument/didChange" => {
                        let params: DidChangeTextDocumentParams = serde_json::from_value(not.params)?;
                        let doc_id = params.text_document; // This is VersionedTextDocumentIdentifier
                        if let Some(change) = params.content_changes.into_iter().next() {
                            eprintln!("LSP: Changed file: {:?}", doc_id.uri);
                            let document_state = DocumentState {
                                uri: doc_id.uri.clone(),
                                content: change.text.clone(),
                                version: Some(doc_id.version), // doc_id.version is i32
                            };
                            open_documents_arc.lock().unwrap().insert(doc_id.uri.clone(), document_state);
                            let current_settings = settings_arc.lock().unwrap().clone(); // Clone settings
                            publish_diagnostics_for_uri(&connection, doc_id.uri, &change.text, &current_settings)?;
                        }
                    }
                    "textDocument/didSave" => {
                        let params: DidSaveTextDocumentParams = serde_json::from_value(not.params)?;
                        let doc_id = params.text_document; // This is TextDocumentIdentifier
                        eprintln!("LSP: Saved file: {:?}", doc_id.uri);
                        if let Some(doc_state) = open_documents_arc.lock().unwrap().get(&doc_id.uri) {
                            let current_settings = settings_arc.lock().unwrap().clone(); // Clone settings
                            publish_diagnostics_for_uri(&connection, doc_id.uri.clone(), &doc_state.content, &current_settings)?;
                        } else {
                            eprintln!("LSP Warning: didSave received for unknown document: {:?}", doc_id.uri);
                        }
                    }
                    "exit" => {
                        eprintln!("LSP: Exit notification received.");
                        return Ok(());
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
    settings: &Settings,
) -> anyhow::Result<()> {
    eprintln!("LSP: Publishing diagnostics for: {:?}", uri);
    let diagnostics = generate_diagnostics(uri.clone(), content, settings);
    connection.sender.send(Message::Notification(LspServerNotification {
        method: lsp_types::notification::PublishDiagnostics::METHOD.to_string(), // Correct usage
        params: serde_json::to_value(PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        })?,
    }))?;
    Ok(())
}

fn generate_diagnostics(uri: Uri, content: &str, settings: &Settings) -> Vec<Diagnostic> {
    // ... (generate_diagnostics function including file_uri_to_pathbuf remains the same as the corrected one from previous response)
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
        Ok(_) => { eprintln!("LSP: Background compilation successful for {:?}.", temp_file_path); }
        Err(CrbrsError::CompilationFailed { errors, .. }) => {
            eprintln!("LSP: Background compilation of {:?} failed. {} errors found.", temp_file_path, errors.len());
            for err_detail in errors {
                let line_0_indexed = err_detail.line.unwrap_or(1).saturating_sub(1);
                let range = Range { start: Position { line: line_0_indexed, character: 0 }, end: Position { line: line_0_indexed, character: u32::MAX }, };
                diagnostics.push(Diagnostic { range, severity: Some(DiagnosticSeverity::ERROR), source: Some("crbrs-compiler".to_string()), message: err_detail.message, ..Default::default() });
            }
        }
        Err(other_crbrs_error) => {
            eprintln!("LSP: Error during background compilation for {:?}: {}", temp_file_path, other_crbrs_error);
            diagnostics.push(Diagnostic { range: Range::default(), severity: Some(DiagnosticSeverity::ERROR), source: Some("crbrs-lsp".to_string()), message: format!("crbrs tool error during compilation: {}", other_crbrs_error), ..Default::default() });
        }
    }
    diagnostics
}