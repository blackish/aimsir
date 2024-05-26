use clap;
use log;
use simple_logger;
use std::net::IpAddr;
use aimsir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = clap::Command::new("aimsir-client")
        .version("0.0.1")
        .arg(clap::arg!(id: -i --id <id> "node id")
            .required(true))
        .arg(clap::arg!(ip: -p --ip <ip> "node local ip")
            .required(true))
        .arg(clap::arg!(server: -s --server <server> "manager server")
            .required(true))
        .arg(clap::arg!(loglevel: -l --loglevel <LOGLEVEL> "loglevel")
            .value_parser([
                clap::builder::PossibleValue::new("error"),
                clap::builder::PossibleValue::new("warn"),
                clap::builder::PossibleValue::new("info"),
                clap::builder::PossibleValue::new("debug"),
            ]))
        .get_matches();
    match app.get_one::<String>("loglevel").unwrap_or(&String::from("info")).as_str() {
        "error" => {
            simple_logger::init_with_level(log::Level::Error).unwrap();
        },
        "warn" => {
            simple_logger::init_with_level(log::Level::Warn).unwrap();
        },
        "info" => {
            simple_logger::init_with_level(log::Level::Info).unwrap();
        },
        "debug" => {
            simple_logger::init_with_level(log::Level::Debug).unwrap();
        },
        _ => {
            simple_logger::init_with_level(log::Level::Info).unwrap();
        }
    }
    let node_id = app.get_one::<String>("id").unwrap().as_str();
    let node_ip = app.get_one::<String>("ip").unwrap().as_str();
    let server = app.get_one::<String>("server").unwrap().as_str();
    if let Ok(_parsed_ip) = node_ip.parse::<IpAddr>() {
        let mut aimsir_client = aimsir::client_manager::ManagerController::new(
            server.into(), node_id.into(), node_ip.into()
        ).await?;
        aimsir_client.worker().await?;
    }
    Ok(())
}
