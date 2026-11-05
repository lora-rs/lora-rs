//! Runner-side half of the rig daemon's datagram relay: replays bench-side
//! gateway datagrams onto the local harness socket and returns the harness's
//! answers, so `Harness` binds :1730 exactly as it does on the bench.
//!
//! Each bench-side peer (gwmp-mux uses separate sockets for PUSH and PULL
//! traffic) gets its own local UDP socket, so the harness sees distinct
//! source addresses and its replies route back to the right peer.

use banc_host::Node;
use lora_hil_relay_icd::{FromBenchTopic, ToBench, ToBenchEndpoint};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::task::JoinSet;

pub struct RelayPump {
    // Held so the node connection outlives the pump tasks.
    _node: Node,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for RelayPump {
    fn drop(&mut self) {
        // Aborting the pump task drops its JoinSet, which aborts the
        // per-peer return paths with it.
        self.task.abort();
    }
}

impl RelayPump {
    /// Subscribe to bench datagrams on an established rig-daemon node and
    /// start pumping them at `harness` (the local `Harness::bind` address).
    pub async fn start(node: Node, harness: SocketAddr) -> anyhow::Result<RelayPump> {
        let mut sub = node
            .client()
            .subscribe_multi::<FromBenchTopic>(64)
            .await
            .map_err(|e| anyhow::anyhow!("subscribing to relay topic: {e:?}"))?;
        let client = node.client().clone();

        let task = tokio::spawn(async move {
            let mut peers: HashMap<u32, Arc<UdpSocket>> = HashMap::new();
            let mut return_paths = JoinSet::new();
            loop {
                let msg = match sub.recv().await {
                    Ok(m) => m,
                    // Closed subscription: the node connection died; the next
                    // scenario's fresh pump will report it as a bind failure.
                    Err(_) => return,
                };
                let sock = match peers.get(&msg.peer) {
                    Some(s) => s.clone(),
                    None => {
                        let sock = match UdpSocket::bind("127.0.0.1:0").await {
                            Ok(s) => Arc::new(s),
                            Err(e) => {
                                println!("relay: local socket for peer {}: {e}", msg.peer);
                                continue;
                            }
                        };
                        if let Err(e) = sock.connect(harness).await {
                            println!("relay: connecting peer {} to harness: {e}", msg.peer);
                            continue;
                        }
                        let (peer, client, back) = (msg.peer, client.clone(), sock.clone());
                        return_paths.spawn(async move {
                            let mut buf = vec![0u8; 2048];
                            loop {
                                let n = match back.recv(&mut buf).await {
                                    Ok(n) => n,
                                    Err(_) => return,
                                };
                                let req = ToBench { peer, data: buf[..n].to_vec() };
                                if client.send_resp::<ToBenchEndpoint>(&req).await.is_err() {
                                    return;
                                }
                            }
                        });
                        peers.entry(msg.peer).or_insert(sock).clone()
                    }
                };
                if let Err(e) = sock.send(&msg.data).await {
                    println!("relay: forwarding to harness for peer {}: {e}", msg.peer);
                }
            }
        });

        Ok(RelayPump { _node: node, task })
    }
}
