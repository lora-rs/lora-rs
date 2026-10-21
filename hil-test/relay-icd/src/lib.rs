//! ICD for the lora-hil rig daemon's datagram relay.
//!
//! The daemon owns the bench-side UDP socket the gateway mux sends to and
//! relays opaque datagrams to whichever runner holds the node connection; the
//! runner's fixture replays them onto its local harness socket (and back).
//! Neither side parses GWMP here: peers are identified by an opaque id so the
//! runner can keep bench-side sockets distinct (Semtech gateways use separate
//! sockets for PUSH and PULL traffic) without either end knowing that.

use postcard_rpc::{TopicDirection, endpoints, topics};
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

/// A datagram from the runner to a bench-side peer, addressed by the peer id
/// a prior [`FromBench`] carried.
#[derive(Serialize, Deserialize, Schema, Debug, Clone)]
pub struct ToBench {
    pub peer: u32,
    pub data: Vec<u8>,
}

/// A datagram a bench-side peer sent to the rig's relay socket. Peer ids are
/// stable per source address for the daemon's lifetime.
#[derive(Serialize, Deserialize, Schema, Debug, Clone)]
pub struct FromBench {
    pub peer: u32,
    pub data: Vec<u8>,
}

endpoints! {
    list = ENDPOINT_LIST;
    | EndpointTy       | RequestTy | ResponseTy | Path                       |
    | ----------       | --------- | ---------- | ----                       |
    | ToBenchEndpoint  | ToBench   | ()         | "lora-hil/relay/to-bench"  |
}

topics! {
    list = TOPICS_OUT_LIST;
    direction = TopicDirection::ToClient;
    | TopicTy          | MessageTy | Path                        |
    | -------          | --------- | ----                        |
    | FromBenchTopic   | FromBench | "lora-hil/relay/from-bench" |
}

topics! {
    list = TOPICS_IN_LIST;
    direction = TopicDirection::ToServer;
    | TopicTy          | MessageTy | Path                        |
    | -------          | --------- | ----                        |
}
