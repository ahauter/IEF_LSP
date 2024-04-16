use log::{error, info, warn, LevelFilter, Log};
use lsp_server::{
    Connection, ExtractError, Message, Notification, Request, RequestId, Response, ResponseError,
};
use lsp_types::{
    notification, Diagnostic, DiagnosticOptions, DiagnosticServerCapabilities, OneOf, Position,
    TextDocumentIdentifier,
};
use lsp_types::{
    DocumentDiagnosticReport, DocumentDiagnosticReportKind, FullDocumentDiagnosticReport,
    InitializeParams, PublishDiagnosticsParams, RelatedFullDocumentDiagnosticReport,
    ServerCapabilities, TextDocumentContentChangeEvent, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url, VersionedTextDocumentIdentifier,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::str::FromStr;
use workspace::IEF_Workspace;
mod workspace;

struct SocketLogger {}

impl SocketLogger {
    fn socket(&self) -> UnixStream {
        return UnixStream::connect("/tmp/debug.socket").unwrap();
    }
}

impl Log for SocketLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        let protocol_version = 1;
        let s = format!("{}\n{}\n", protocol_version, record.args());
        let mut sock = self.socket();
        sock.write_all(s.as_bytes());
        sock.flush();
    }
    fn flush(&self) {}
}

const fn init_logger() -> SocketLogger {
    SocketLogger {}
}

#[derive(Debug)]
struct ServerError {
    msg: String,
}
impl Error for ServerError {}
impl Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Server Error: ");
        f.write_str(self.msg.as_str())
    }
}

const LOGGER: SocketLogger = init_logger();
fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();
    log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info));
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        definition_provider: None,
        diagnostic_provider: Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
            identifier: None,
            inter_file_dependencies: true,
            workspace_diagnostics: false,
            work_done_progress_options: lsp_types::WorkDoneProgressOptions {
                work_done_progress: None,
            },
        })),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::INCREMENTAL,
        )),
        ..Default::default()
    })
    .unwrap();
    info!("Starting IEF_LSP V2! :)");
    let init_params = connection.initialize(server_capabilities).unwrap();
    let _ = main_loop(connection, init_params);
    io_threads.join().expect("Threads are frayed");
    //SHut down
    info!("IEF_LSP V2 Stopped :(");
    Ok(())
}

fn handle_request(workspace: &mut IEF_Workspace, req: Request) -> Vec<Message> {
    info!("Got request {:?}", req);
    match req.method.as_str() {
        "textDocument/diagnostic" => {
            let doc_uri = match req.params.get("textDocument") {
                Some(doc) => doc.get("uri"),
                None => None,
            };
            if doc_uri.is_none() {
                return vec![Message::Response(Response {
                    id: req.id,
                    result: None,
                    error: Some(ResponseError {
                        code: 400,
                        message: String::from("document uri is not defined"),
                        data: None,
                    }),
                })];
            };
            let doc_uri = doc_uri.unwrap().as_str().unwrap().to_string();
            let mut diagnostic_req_res = workspace.get_diagnostics();
            let doc_diagnostics = diagnostic_req_res.remove(&doc_uri).unwrap_or(vec![]);
            let mess = Message::Response(Response {
                id: req.id.clone(),
                result: Some(
                    serde_json::to_value(DocumentDiagnosticReport::Full(
                        RelatedFullDocumentDiagnosticReport {
                            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                                result_id: Some(req.id.to_string().clone()),
                                items: doc_diagnostics,
                            },
                            related_documents: Some(HashMap::from_iter(
                                diagnostic_req_res.iter().map(|(uri_str, diag_vec)| {
                                    (
                                        Url::from_str(uri_str).unwrap(),
                                        DocumentDiagnosticReportKind::Full(
                                            FullDocumentDiagnosticReport {
                                                result_id: Some(req.id.to_string().clone()),
                                                items: diag_vec.to_owned(),
                                            },
                                        ),
                                    )
                                }),
                            )),
                        },
                    ))
                    .unwrap(),
                ),
                error: None,
            });

            info!("Diagnoistics req result {:?}", mess);
            return vec![mess];
        }
        _ => {
            info!("Unsupported method! {req:?}");
        }
    };
    return vec![];
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentChangeNotification {
    content_changes: Vec<TextDocumentContentChangeEvent>,
    text_document: TextDocumentIdentifier,
}
fn handle_notification(worksp: &mut IEF_Workspace, not: Notification) -> Vec<Message> {
    match not.method.as_str() {
        "textDocument/didSave" => {
            let results: Vec<_> = worksp
                .get_diagnostics()
                .iter()
                .map(|(uri, diags)| PublishDiagnosticsParams {
                    uri: Url::from_str(uri).unwrap(),
                    diagnostics: diags.to_owned(),
                    version: None,
                })
                .map(|diag_params| {
                    Message::Notification(Notification {
                        method: String::from("textDocument/publishDiagnostics"),
                        params: serde_json::to_value(diag_params).unwrap(),
                    })
                })
                .collect();
            info!("Save diagnostics results: {:?}", results);
            return results;
        }
        "textDocument/didClose" => info!("{:?}", not.method),
        "textDocument/didOpen" => info!("{:?}", not.method),
        "textDocument/didChange" => {
            info!("{:?}", not);
            let edit_param: DocumentChangeNotification =
                serde_json::from_value(not.params).unwrap();
            worksp.update_document(edit_param.text_document.uri, edit_param.content_changes);
        }
        _ => info!("Method not implemented {:?}", not.method),
    }
    return vec![];
}

fn main_loop(
    connection: Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let params: InitializeParams = serde_json::from_value(params).unwrap();
    let root_uri = match params.root_uri {
        Some(url) => String::from(url.as_str()),
        None => {
            error!("Root URI is None!");
            return Err(Box::new(ServerError {
                msg: String::from("Root URI is NonE!"),
            }));
        }
    };
    let mut workspace = workspace::new_workspace(root_uri.as_str());
    info!("Created workspace representation");
    info!("Starting Main loop!");
    for msg in &connection.receiver {
        let result = match msg {
            Message::Request(req) => {
                if req.method == "shutdown" {
                    break;
                }
                handle_request(&mut workspace, req)
            }
            Message::Notification(not) => handle_notification(&mut workspace, not),
            _ => {
                warn!("This must be a response");
                vec![]
            }
        };
        for msg in result {
            let res = connection.sender.send(msg);
            info!("Sent message to client {:?}", res);
        }
    }

    info!("Main loop over");
    Ok(())
}
