use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use warp::ws::{Message, WebSocket};
use warp::Filter;
type StreamId = String;
type Clients = Arc<RwLock<HashMap<StreamId, Vec<mpsc::UnboundedSender<Message>>>>>;

#[tokio::main]
async fn main() {
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));

    // Route for video publishers
    let publish = warp::path!("publish" / StreamId)
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .map(|stream_id: StreamId, ws: warp::ws::Ws, clients| {
            ws.on_upgrade(move |socket| handle_publisher(socket, stream_id, clients))
        });

    // Route for video consumers
    let consume = warp::path!("consume" / StreamId)
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .map(|stream_id: StreamId, ws: warp::ws::Ws, clients| {
            ws.on_upgrade(move |socket| handle_consumer(socket, stream_id, clients))
        });
    // Route to serve the player HTML
    // Route to serve the publisher HTML
    let index = warp::path("static").and(warp::filters::fs::dir("static"));
    // Combine routes
    let routes = publish
        .or(consume)
        .or(index)
        .with(warp::cors().allow_any_origin());
    println!("Server started on http://localhost:3030");
    warp::serve(routes)
        .tls()
        .cert_path("./belo.chat/cert4.pem")
        .key_path("./belo.chat/privkey4.pem")
        .run(([0, 0, 0, 0], 3040))
        .await;
}

fn with_clients(
    clients: Clients,
) -> impl Filter<Extract = (Clients,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || clients.clone())
}

async fn handle_publisher(ws: WebSocket, stream_id: StreamId, clients: Clients) {
    println!("New publisher connected for stream: {}", stream_id);

    let (_ws_tx, mut ws_rx) = ws.split();

    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                let clients_read = clients.read().await;
                if let Some(stream_clients) = clients_read.get(&stream_id) {
                    for client in stream_clients {
                        let _ = client.send(msg.clone());
                    }
                }
            }
            Err(e) => {
                eprintln!("Error receiving message from publisher: {:?}", e);
                break;
            }
        }
    }

    println!("Publisher disconnected for stream: {}", stream_id);
}

async fn handle_consumer(ws: WebSocket, stream_id: StreamId, clients: Clients) {
    println!("New consumer connected for stream: {}", stream_id);

    let (mut ws_tx, mut ws_rx) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Add the client to the stream's client list
    {
        let mut clients_write = clients.write().await;
        clients_write
            .entry(stream_id.clone())
            .or_insert_with(Vec::new)
            .push(tx);
    }

    // Handle incoming messages (if any)
    let incoming = tokio::spawn(async move {
        while let Some(result) = ws_rx.next().await {
            if let Err(e) = result {
                eprintln!("Error receiving message from consumer: {:?}", e);
                break;
            }
        }
    });

    // Send video chunks to the client
    let outgoing = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = ws_tx.send(msg).await {
                eprintln!("Error sending message to consumer: {:?}", e);
                break;
            }
        }
    });

    // Wait for either incoming or outgoing to finish
    tokio::select! {
        _ = incoming => {},
        _ = outgoing => {},
    }

    println!("Consumer disconnected from stream: {}", stream_id);

    // Remove the client from the stream's client list
    let mut clients_write = clients.write().await;
    if let Some(stream_clients) = clients_write.get_mut(&stream_id) {
        stream_clients.retain(|client| client.is_closed());
        if stream_clients.is_empty() {
            clients_write.remove(&stream_id);
        }
    }
}
