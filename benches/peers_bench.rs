use tokio::sync::mpsc;
use std::{collections::HashMap, time};

use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};

use aimsir::{peers_controller, model};

async fn peers_measurements(peers_num: usize) {
    let (tx, rx) = mpsc::channel(1);
    let (mgr_tx, _mgr_rx) = mpsc::channel(1);
    let mut neighbour_update = model::NeighbourUpdate{
        update_type: model::UpdateType::Add,
        probe_timer: 1,
        aggregate_timer: 10,
        update: Vec::new()
    };
    for peer_id in [..peers_num] {
        neighbour_update.update.push(
            model::aimsir::Peer{
                id: format!("{:?}", peer_id).into(),
                ipaddress: "127.0.0.1".into()
            }
        )
    }
    let mut measurements: HashMap<String, model::Measurement> = HashMap::new();
    let ctrl = peers_controller::PeerController::new(
        "0".into(),
        3600,
        36000,
        rx,
        mgr_tx,
        true
    ).await;
    let _ = tx.send(neighbour_update).await;
    ctrl.make_aggregate(&mut measurements, time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_millis() as u64);
}

fn bench_peers(c: &mut Criterion) {
    let size: usize = 1_024;

    c.bench_with_input(
        BenchmarkId::new(
            "calc_aggregates", size
        ),
        &size,
        |b, &s|
        {
        // Insert a call to `to_async` to convert the bencher to async mode.
        // The timing loops are the same as with the normal bencher.
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| peers_measurements(s));
        }
    );
}

criterion_group!(benches, bench_peers);
criterion_main!(benches);
