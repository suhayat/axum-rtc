use std::net::IpAddr;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use crate::room::Rooms;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use mediasoup::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
pub struct SignalingMessage {
    pub id: Option<u64>,
    pub method: Option<String>,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Serialize)]
pub struct SignalingResponse {
    pub id: Option<u64>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ServerNotification {
    pub method: String,
    pub data: Value,
}

pub type PeerChannels = Arc<TokioMutex<HashMap<String, HashMap<String, mpsc::UnboundedSender<String>>>>>;

pub static PEER_CHANNELS: LazyLock<PeerChannels> = LazyLock::new(|| {
    Arc::new(TokioMutex::new(HashMap::new()))
});

pub async fn register_peer_channel(room_id: &str, peer_id: &str, tx: mpsc::UnboundedSender<String>) {
    let mut channels = PEER_CHANNELS.lock().await;
    channels.entry(room_id.to_string())
        .or_insert_with(HashMap::new)
        .insert(peer_id.to_string(), tx);
}

pub async fn unregister_peer_channel(room_id: &str, peer_id: &str) {
    let mut channels = PEER_CHANNELS.lock().await;
    if let Some(room_channels) = channels.get_mut(room_id) {
        room_channels.remove(peer_id);
        if room_channels.is_empty() {
            channels.remove(room_id);
        }
    }
}

pub async fn broadcast_to_room_exclude(room_id: &str, exclude_peer: &str, message: &str) {
    let channels = PEER_CHANNELS.lock().await;
    if let Some(room_channels) = channels.get(room_id) {
        for (pid, tx) in room_channels {
            if pid != exclude_peer {
                let _ = tx.send(message.to_string());
            }
        }
    }
}

pub async fn handle_ws(socket: WebSocket, room_id: String, peer_id: String, rooms: Rooms, router: Router) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Forward messages from channel to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(Message::Text(msg.clone().into())).await.is_err() {
                break;
            }
        }
    });

    // Register peer
    register_peer_channel(&room_id, &peer_id, tx.clone()).await;

    // Process incoming signaling messages
    while let Some(Ok(msg)) = ws_rx.next().await {
        if let Message::Text(text) = msg {
            if let Ok(sig_msg) = serde_json::from_str::<SignalingMessage>(&text) {
                let response = handle_signaling_message(
                    &sig_msg,
                    &room_id,
                    &peer_id,
                    &rooms,
                    &router,
                ).await;

                if let Some(resp) = response {
                    let resp_str = serde_json::to_string(&resp).unwrap();
                    let _ = tx.send(resp_str);
                }
            } else {
                warn!("Failed to parse signaling message: {}", text);
            }
        }
    }

    // Cleanup
    unregister_peer_channel(&room_id, &peer_id).await;
    
    // Notify others
    let notification = ServerNotification {
        method: "peerClosed".to_string(),
        data: json!({ "peerId": peer_id }),
    };
    if let Ok(notif_str) = serde_json::to_string(&notification) {
        broadcast_to_room_exclude(&room_id, &peer_id, &notif_str).await;
    }

    {
        let mut rooms_lock = rooms.lock().await;
        if let Some(room) = rooms_lock.get_mut(&room_id) {
            room.remove_peer(&peer_id);
            if room.peers.is_empty() {
                rooms_lock.remove(&room_id);
            }
        }
    }

    drop(tx);
    let _ = send_task.await;
}

async fn handle_signaling_message(
    msg: &SignalingMessage,
    room_id: &str,
    peer_id: &str,
    rooms: &Rooms,
    router: &Router,
) -> Option<SignalingResponse> {
    let method = match msg.method.as_deref() {
        Some(m) => m,
        None => return None,
    };
    let id = msg.id;

    info!("Handling signaling method: {} from peer: {}", method, peer_id);

    match method {
        "getRouterRtpCapabilities" => {
            let caps = router.rtp_capabilities().clone();
            Some(SignalingResponse {
                id,
                ok: true,
                data: Some(serde_json::to_value(caps).unwrap()),
                error: None,
            })
        }
        "createWebRtcTransport" => {
            let direction = msg.data.get("direction").and_then(|v| v.as_str()).unwrap_or("send");

            // GANTI "127.0.0.1" dengan IP lokal komputer Anda (MISAL: "192.168.1.10")
            // agar bisa diakses oleh browser berbeda atau perangkat lain.
            let ip_listening = "0.0.0.0".parse::<IpAddr>().unwrap();
            let announced_address = std::env::var("ANNOUNCED_IP").expect("ANNOUNCED_IP must be set");

            let mut options = WebRtcTransportOptions::new(WebRtcTransportListenInfos::new(ListenInfo {
                protocol: Protocol::Udp,
                ip: ip_listening.into(),
                announced_address: Some(announced_address),
                expose_internal_ip: false,
                port: None,
                port_range: Some(10000..=10100),
                flags: None,
                send_buffer_size: None,
                recv_buffer_size: None,
            }));
            options.enable_udp = true;
            options.enable_tcp = true;
            options.prefer_udp = true;

            match router.create_webrtc_transport(options).await {
                Ok(transport) => {
                    let resp_data = json!({
                        "id": transport.id().to_string(),
                        "iceParameters": transport.ice_parameters(),
                        "iceCandidates": transport.ice_candidates(),
                        "dtlsParameters": transport.dtls_parameters(),
                    });

                    let mut rooms_lock = rooms.lock().await;
                    if let Some(room) = rooms_lock.get_mut(room_id) {
                        if let Some(peer) = room.get_peer_mut(peer_id) {
                            if direction == "send" {
                                peer.send_transport = Some(transport);
                            } else {
                                peer.recv_transport = Some(transport);
                            }
                        }
                    }

                    Some(SignalingResponse {
                        id,
                        ok: true,
                        data: Some(resp_data),
                        error: None,
                    })
                }
                Err(e) => Some(SignalingResponse {
                    id,
                    ok: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            }
        }
        "connectTransport" => {
            let transport_id = msg.data.get("transportId").and_then(|v| v.as_str()).unwrap_or("");
            let dtls_parameters: DtlsParameters = match serde_json::from_value(
                msg.data.get("dtlsParameters").cloned().unwrap_or(Value::Null)
            ) {
                Ok(p) => p,
                Err(e) => return Some(SignalingResponse { id, ok: false, data: None, error: Some(format!("Invalid DTLS parameters: {}", e)) }),
            };

            let mut rooms_lock = rooms.lock().await;
            if let Some(room) = rooms_lock.get_mut(room_id) {
                if let Some(peer) = room.get_peer_mut(peer_id) {
                    let transport = peer.send_transport.as_ref()
                        .filter(|t| t.id().to_string() == transport_id)
                        .or_else(|| peer.recv_transport.as_ref()
                            .filter(|t| t.id().to_string() == transport_id));

                    if let Some(t) = transport {
                        match t.connect(WebRtcTransportRemoteParameters { dtls_parameters }).await {
                            Ok(_) => return Some(SignalingResponse { id, ok: true, data: None, error: None }),
                            Err(e) => return Some(SignalingResponse { id, ok: false, data: None, error: Some(e.to_string()) }),
                        }
                    }
                }
            }
            Some(SignalingResponse { id, ok: false, data: None, error: Some("Transport not found".to_string()) })
        }
        "produce" => {
            let kind: MediaKind = match serde_json::from_value(msg.data.get("kind").cloned().unwrap_or(Value::Null)) {
                Ok(k) => k,
                Err(e) => return Some(SignalingResponse { id, ok: false, data: None, error: Some(format!("Invalid media kind: {}", e)) }),
            };
            let rtp_parameters: RtpParameters = match serde_json::from_value(msg.data.get("rtpParameters").cloned().unwrap_or(Value::Null)) {
                Ok(p) => p,
                Err(e) => return Some(SignalingResponse { id, ok: false, data: None, error: Some(format!("Invalid RTP parameters: {}", e)) }),
            };

            let mut rooms_lock = rooms.lock().await;
            if let Some(room) = rooms_lock.get_mut(room_id) {
                if let Some(peer) = room.get_peer_mut(peer_id) {
                    if let Some(transport) = &peer.send_transport {
                        match transport.produce(ProducerOptions::new(kind, rtp_parameters)).await {
                            Ok(producer) => {
                                let producer_id = producer.id();
                                peer.producers.insert(producer_id, producer);

                                // Notify others
                                let notification = ServerNotification {
                                    method: "newProducer".to_string(),
                                    data: json!({
                                        "peerId": peer_id,
                                        "producerId": producer_id.to_string(),
                                        "kind": serde_json::to_value(&kind).unwrap(),
                                    }),
                                };
                                if let Ok(notif_str) = serde_json::to_string(&notification) {
                                    broadcast_to_room_exclude(room_id, peer_id, &notif_str).await;
                                }

                                return Some(SignalingResponse {
                                    id,
                                    ok: true,
                                    data: Some(json!({ "producerId": producer_id.to_string() })),
                                    error: None,
                                });
                            }
                            Err(e) => return Some(SignalingResponse { id, ok: false, data: None, error: Some(e.to_string()) }),
                        }
                    }
                }
            }
            Some(SignalingResponse { id, ok: false, data: None, error: Some("Send transport not found".to_string()) })
        }
        "consume" => {
            let producer_id_str = msg.data.get("producerId").and_then(|v| v.as_str()).unwrap_or("");
            let rtp_capabilities: RtpCapabilities = match serde_json::from_value(msg.data.get("rtpCapabilities").cloned().unwrap_or(Value::Null)) {
                Ok(c) => c,
                Err(e) => return Some(SignalingResponse { id, ok: false, data: None, error: Some(format!("Invalid RTP capabilities: {}", e)) }),
            };
            let producer_id: ProducerId = match producer_id_str.parse() {
                Ok(pid) => pid,
                Err(e) => return Some(SignalingResponse { id, ok: false, data: None, error: Some(format!("Invalid producer ID format: {}", e)) }),
            };

            if !router.can_consume(&producer_id, &rtp_capabilities) {
                warn!("Router cannot consume producer {}. Codec mismatch?", producer_id);
                return Some(SignalingResponse { id, ok: false, data: None, error: Some("Cannot consume (codec mismatch or producer not found)".to_string()) });
            }

            let mut rooms_lock = rooms.lock().await;
            if let Some(room) = rooms_lock.get_mut(room_id) {
                // Debug log: verify producer exists in the room
                let producer_exists = room.get_producer_peer_ids("").iter().any(|(_, pid, _)| pid == producer_id_str);
                info!("Consumer request: room={}, peer={}, target_producer={}, exists={}", room_id, peer_id, producer_id_str, producer_exists);

                if let Some(peer) = room.get_peer_mut(peer_id) {
                    if let Some(transport) = &peer.recv_transport {
                        let mut options = ConsumerOptions::new(producer_id, rtp_capabilities);
                        options.paused = true;
                        match transport.consume(options).await {
                            Ok(consumer) => {
                                let consumer_id = consumer.id();
                                let kind = consumer.kind();
                                let rtp_params = consumer.rtp_parameters().clone();
                                peer.consumers.insert(consumer_id, consumer);

                                return Some(SignalingResponse {
                                    id,
                                    ok: true,
                                    data: Some(json!({
                                        "consumerId": consumer_id.to_string(),
                                        "producerId": producer_id_str,
                                        "kind": serde_json::to_value(&kind).unwrap(),
                                        "rtpParameters": rtp_params,
                                    })),
                                    error: None,
                                });
                            }
                            Err(e) => return Some(SignalingResponse { id, ok: false, data: None, error: Some(e.to_string()) }),
                        }
                    }
                }
            }
            Some(SignalingResponse { id, ok: false, data: None, error: Some("Recv transport not found".to_string()) })
        }
        "resumeConsumer" => {
            let consumer_id_str = msg.data.get("consumerId").and_then(|v| v.as_str()).unwrap_or("");
            let consumer_id: ConsumerId = match consumer_id_str.parse() {
                Ok(cid) => cid,
                Err(e) => return Some(SignalingResponse { id, ok: false, data: None, error: Some(format!("Invalid consumer ID format: {}", e)) }),
            };

            let rooms_lock = rooms.lock().await;
            if let Some(room) = rooms_lock.get(room_id) {
                if let Some(peer) = room.get_peer(peer_id) {
                    if let Some(consumer) = peer.consumers.get(&consumer_id) {
                        match consumer.resume().await {
                            Ok(_) => return Some(SignalingResponse { id, ok: true, data: None, error: None }),
                            Err(e) => return Some(SignalingResponse { id, ok: false, data: None, error: Some(e.to_string()) }),
                        }
                    }
                }
            }
            Some(SignalingResponse { id, ok: false, data: None, error: Some("Consumer not found".to_string()) })
        }
        "getProducers" => {
            let rooms_lock = rooms.lock().await;
            let producers = if let Some(room) = rooms_lock.get(room_id) {
                room.get_producer_peer_ids(peer_id)
            } else {
                vec![]
            };

            let producers_json: Vec<Value> = producers.iter().map(|(pid, prod_id, kind)| {
                json!({ "peerId": pid, "producerId": prod_id, "kind": kind })
            }).collect();

            Some(SignalingResponse { id, ok: true, data: Some(json!(producers_json)), error: None })
        }
        _ => {
            warn!("Unknown method: {}", method);
            Some(SignalingResponse { id, ok: false, data: None, error: Some(format!("Unknown method: {}", method)) })
        }
    }
}
