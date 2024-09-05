use aimsir::backend_manager::{render_results, stats, stats_id};
use aimsir::{
    self,
    backend_manager::BackendState,
    model::{self, aimsir::aimsir_service_server::AimsirServiceServer},
};
use axum::{routing::get, Router};
use clap;
use log;
use simple_logger;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

#[tokio::main]
// #[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = clap::Command::new("aimsir-server")
        .version("0.0.1")
        .arg(clap::arg!(ip: -p --ip <ip> "node local ip").required(true))
        .arg(clap::arg!(webip: -w --web-ip <webip> "node web ui local ip").required(true))
        .arg(
            clap::arg!(interval: -i --interval <interval> "probe interval")
                .required(true)
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            clap::arg!(aggregate: -a --aggregate <aggregate> "aggregate interval")
                .required(true)
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            clap::arg!(loglevel: -l --loglevel <LOGLEVEL> "loglevel").value_parser([
                clap::builder::PossibleValue::new("error"),
                clap::builder::PossibleValue::new("warn"),
                clap::builder::PossibleValue::new("info"),
                clap::builder::PossibleValue::new("debug"),
            ]),
        )
        .get_matches();
    match app
        .get_one::<String>("loglevel")
        .unwrap_or(&String::from("info"))
        .as_str()
    {
        "error" => {
            simple_logger::init_with_level(log::Level::Error).unwrap();
        }
        "warn" => {
            simple_logger::init_with_level(log::Level::Warn).unwrap();
        }
        "info" => {
            simple_logger::init_with_level(log::Level::Info).unwrap();
        }
        "debug" => {
            simple_logger::init_with_level(log::Level::Debug).unwrap();
        }
        _ => {
            simple_logger::init_with_level(log::Level::Info).unwrap();
        }
    }
    let node_ip: SocketAddr = app
        .get_one::<String>("ip")
        .unwrap()
        .as_str()
        .parse()
        .unwrap();
    let web_ip: SocketAddr = app
        .get_one::<String>("webip")
        .unwrap()
        .as_str()
        .parse()
        .unwrap();
    let aimsir_server = aimsir::server_manager::ServerController::new(
        *app.get_one::<u32>("interval").expect("Expect u32 interval"),
        *app.get_one::<u32>("aggregate")
            .expect("Expect u32 aggregate interval"),
        Box::new(model::db::SqliteDb::new("test".to_string()).await?),
    )
    .await?;

    let input_metrics = aimsir_server.get_metrics();
    let parse_db = Box::new(model::db::SqliteDb::new("test".to_string()).await?);
    let metrics = Arc::new(RwLock::new(HashMap::new()));
    let reconcile_time: u16 = 60;

    let server = AimsirServiceServer::new(aimsir_server);
    tokio::spawn(async move {
        let _ = tonic::transport::Server::builder()
            .add_service(server)
            .serve(node_ip)
            .await;
    });
    // let web_ip: SocketAddr = app.get_one::<String>("webip").unwrap().as_str().parse().unwrap();
    let web_app = Router::new()
        .route("/stats", get(stats))
        .route("/stats/:statid", get(stats_id))
        .with_state(BackendState {
            metrics: metrics.clone(),
            db: Arc::new(RwLock::new(
                model::db::SqliteDb::new("test".to_string()).await?,
            )),
        });
    tokio::spawn(async move {
        let _ = render_results(input_metrics, parse_db, reconcile_time, metrics).await;
    });
    let listener = TcpListener::bind(web_ip).await.unwrap();
    axum::serve(listener, web_app).await.unwrap();
    Ok(())
}
