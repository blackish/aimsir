use aimsir::backend_manager::{
    add_peer, add_peer_tag, add_tag, del_peer, del_peer_tag, del_tag, disable_peer, enable_peer,
    get_metrics, peer_tags, peers, render_results, stats, stats_id, tags, get_full_metrics,
};
use aimsir::{
    self,
    backend_manager::BackendState,
    model::{self, aimsir::aimsir_service_server::AimsirServiceServer},
};
use axum::{
    routing::{delete, get, post},
    Router,
};
use clap;
use log;
use simple_logger;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
// #[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = clap::Command::new("aimsir-server")
        .version("0.0.1")
        .arg(
            clap::arg!(ip: -p --ip <ip> "node local ip")
                .required(true)
                .env("AIMSIR_IP"),
        )
        .arg(
            clap::arg!(webip: -w --webip <webip> "node web ui local ip")
                .required(true)
                .env("AIMSIR_WEBIP"),
        )
        .arg(
            clap::arg!(db: -b --db <db> "database name")
                .required(true)
                .env("AIMSIR_DATABASE_URL"),
        )
        .arg(
            clap::arg!(interval: -i --interval <interval> "probe interval")
                .required(true)
                .value_parser(clap::value_parser!(u32))
                .env("AIMSIR_INTERVAL"),
        )
        .arg(
            clap::arg!(aggregate: -a --aggregate <aggregate> "aggregate interval")
                .required(true)
                .value_parser(clap::value_parser!(u32))
                .env("AIMSIR_AGGREGATE"),
        )
        .arg(
            clap::arg!(loglevel: -l --loglevel <LOGLEVEL> "loglevel")
                .value_parser([
                    clap::builder::PossibleValue::new("error"),
                    clap::builder::PossibleValue::new("warn"),
                    clap::builder::PossibleValue::new("info"),
                    clap::builder::PossibleValue::new("debug"),
                ])
                .env("AIMSIR_LOGLEVEL"),
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
    let db: String = app.get_one::<String>("db").unwrap().as_str().to_string();
    let aimsir_server = aimsir::server_manager::ServerController::new(
        *app.get_one::<u32>("interval").expect("Expect u32 interval"),
        *app.get_one::<u32>("aggregate")
            .expect("Expect u32 aggregate interval"),
        Box::new(model::mysql::MysqlDb::new(db.clone()).await?),
    )
    .await?;

    let input_metrics = aimsir_server.get_metrics();
    let parse_db = Box::new(model::mysql::MysqlDb::new(db.clone()).await?);
    let metrics = Arc::new(RwLock::new(HashMap::new()));
    let atomic_metrics = Arc::new(RwLock::new(HashMap::new()));
    let reconcile_time: u16 = 60;

    let server = AimsirServiceServer::new(aimsir_server);
    tokio::spawn(async move {
        let _ = tonic::transport::Server::builder()
            .add_service(server)
            .serve(node_ip)
            .await;
    });
    let node_ip: String = app.get_one::<String>("ip").unwrap().to_string();
    let cors = CorsLayer::new()
        .allow_origin(Any) // Allows any origin
        .allow_methods(Any) // Allows any HTTP method
        .allow_headers(Any); // Allows any header
    let web_app = Router::new()
        .route("/aimsir/api/v1/stats", get(stats))
        .route("/aimsir/api/v1/stats/:statid", get(stats_id))
        .route("/aimsir/api/v1/peers", get(peers))
        .route("/aimsir/api/v1/disable-peer/:peer", get(disable_peer))
        .route("/aimsir/api/v1/enable-peer/:peer", get(enable_peer))
        .route("/aimsir/api/v1/peers", post(add_peer))
        .route("/aimsir/api/v1/peers/:peer", delete(del_peer))
        .route("/aimsir/api/v1/tags", get(tags))
        .route("/aimsir/api/v1/tags", post(add_tag))
        .route("/aimsir/api/v1/tags/:tag", delete(del_tag))
        .route("/aimsir/api/v1/peertags", get(peer_tags))
        .route("/aimsir/api/v1/peertags", post(add_peer_tag))
        .route("/aimsir/api/v1/peertags/:peer/:tag", delete(del_peer_tag))
        .route("/metrics", get(get_metrics))
        .route("/fullmetrics", get(get_full_metrics))
        .with_state(BackendState {
            metrics: metrics.clone(),
            db: Arc::new(RwLock::new(model::mysql::MysqlDb::new(db).await?)),
            grpc_server: Arc::new(RwLock::new(format!("http://{}", node_ip))),
            atomic_metrics: atomic_metrics.clone(),
        })
        .layer(cors); // Add the CORS middleware;
    tokio::spawn(async move {
        let _ = render_results(input_metrics, parse_db, reconcile_time, metrics, atomic_metrics).await;
    });
    let listener = TcpListener::bind(web_ip).await.unwrap();
    axum::serve(listener, web_app).await.unwrap();
    Ok(())
}
