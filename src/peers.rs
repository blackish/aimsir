use std::collections::HashMap;
use tokio::sync::mpsc::Receiver;

#[derive(Debug)]
struct PeerMessage {
    ipaddress: Box<str>,
    seq: u64,
    ts: u64
}

#[derive(Debug)]
struct Peer {
    ipaddress: Box<str>,
    last_seq: u64,
    last_ts: u64
}

struct PeerManager {
    peers: HashMap<String, Peer>,
    receiver: Receiver<PeerMessage>
}

impl PeerManager {
    pub fn new(rcv: Receiver<PeerMessage>) -> Self {
        Self {
            peers: HashMap::new(),
            receiver: rcv
        }
    }
    async fn work(&mut self) {
        loop {
            if let Some(new_msg) = self.receiver.recv().await {
                println!("{:?}", new_msg);
            }
        }
    }
}
