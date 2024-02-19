use lsp_server::{Connection, ExtractError, Message, Request, RequestId, Response};
use lsp_types::OneOf;
use lsp_types::{
    request::GotoDefinition, GotoDefinitionResponse, InitializeParams, ServerCapabilities,
};
use std::error::Error;
fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    println!("Hello, world!");
    eprintln!("Starting IEF LSP V2");

    let (connection, io_threads) = Connection::stdio();
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        definition_provider: Some(OneOf::Left(true)),
        ..Default::default()
    })
    .unwrap();
    let init_params = connection.initialize(server_capabilities).unwrap();
    main_loop(connection, init_params);
    io_threads.join();

    //SHut down
    eprintln!("Shutting down server");
    Ok(())
}

fn main_loop(
    connection: Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let params: InitializeParams = serde_json::from_value(params).unwrap();
    eprintln!("Server params: {:?}", params);
    eprintln!("Starting Main loop!");
    for msg in &connection.receiver {
        eprintln!("Got message {:?}", msg);
    }
    Ok(())
}
