use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use mediasoup::prelude::*;

/// A single peer in a room
pub struct Peer {
    pub send_transport: Option<WebRtcTransport>,
    pub recv_transport: Option<WebRtcTransport>,
    pub producers: HashMap<ProducerId, Producer>,
    pub consumers: HashMap<ConsumerId, Consumer>,
}

impl Peer {
    pub fn new(_id: String) -> Self {
        Self {
            send_transport: None,
            recv_transport: None,
            producers: HashMap::new(),
            consumers: HashMap::new(),
        }
    }
}

/// A video conference room
pub struct Room {
    pub peers: HashMap<String, Peer>,
}

impl Room {
    pub fn new(_id: String, _router: Router) -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    pub fn add_peer(&mut self, peer_id: String) {
        self.peers.insert(peer_id.clone(), Peer::new(peer_id));
    }

    pub fn remove_peer(&mut self, peer_id: &str) -> Option<Peer> {
        self.peers.remove(peer_id)
    }

    pub fn get_peer(&self, peer_id: &str) -> Option<&Peer> {
        self.peers.get(peer_id)
    }

    pub fn get_peer_mut(&mut self, peer_id: &str) -> Option<&mut Peer> {
        self.peers.get_mut(peer_id)
    }

    pub fn get_producer_peer_ids(&self, exclude_peer: &str) -> Vec<(String, String, String)> {
        let mut result = Vec::new();
        for (peer_id, peer) in &self.peers {
            if peer_id != exclude_peer {
                for (producer_id, producer) in &peer.producers {
                    let kind_str = match producer.kind() {
                        MediaKind::Audio => "audio",
                        MediaKind::Video => "video",
                    };
                    result.push((
                        peer_id.clone(),
                        producer_id.to_string(),
                        kind_str.to_string(),
                    ));
                }
            }
        }
        result
    }
}

pub type Rooms = Arc<Mutex<HashMap<String, Room>>>;

pub fn create_rooms() -> Rooms {
    Arc::new(Mutex::new(HashMap::new()))
}
