use clap;
use log;
use simple_logger;
use std::net::SocketAddr;
use aimsir::{self, model::aimsir::aimsir_service_server::AimsirServiceServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = clap::Command::new("aimsir-server")
        .version("0.0.1")
        .arg(clap::arg!(ip: -p --ip <ip> "node local ip")
            .required(true))
        .arg(clap::arg!(interval: -i --interval <interval> "probe interval")
            .required(true)
            .value_parser(clap::value_parser!(u32)))
        .arg(clap::arg!(aggregate: -a --aggregate <aggregate> "aggregate interval")
            .required(true)
            .value_parser(clap::value_parser!(u32)))
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
    let node_ip: SocketAddr = app.get_one::<String>("ip").unwrap().as_str().parse().unwrap();
    let aimsir_server = aimsir::server_manager::ServerController::new(
        *app.get_one::<u32>("interval").expect("Expect u32 interval"),
        *app.get_one::<u32>("aggregate").expect("Expect u32 aggregate interval"),
    ).await?;
    let server = AimsirServiceServer::new(aimsir_server);
    let _ = tonic::transport::Server::builder().add_service(server).serve(node_ip).await;
    Ok(())
}
