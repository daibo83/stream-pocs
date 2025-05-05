
use bytes::BytesMut;
use bytesio::bytes_reader::BytesReader;
use futures::{SinkExt, StreamExt};
use annexb_2_hecv::{convert_annexb_to_length_prefixed, create_hvcc_box, EXAMPLE_ANNEXB_DATA};
use xflv::mpeg4_hevc::Mpeg4HevcProcessor;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use warp::ws::{Message, WebSocket};
use warp::Filter;
type StreamId = String;
type Clients = Arc<RwLock<HashMap<StreamId, Vec<mpsc::UnboundedSender<Message>>>>>;
type CodecDescriptions = Arc<RwLock<HashMap<StreamId, Vec<u8>>>>;
mod annexb_2_hecv;
#[tokio::main]
async fn main() {
    let mut xflv = Mpeg4HevcProcessor::default();
    let data = EXAMPLE_ANNEXB_DATA[..].to_vec();
    let bytes_mut = BytesMut::from(data.as_slice());
    let mut bytes_reader = BytesReader::new(bytes_mut);
    xflv.decoder_configuration_record_load(&mut bytes_reader).unwrap();
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));
    let codec_descriptions: Arc<RwLock<HashMap<StreamId, Vec<u8>>>> = Arc::new(RwLock::new(HashMap::new()));
    // Route for video publishers
    let publish = warp::path!("publish" / StreamId)
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .and(with_codec_descriptions(codec_descriptions.clone()))
        .map(|stream_id: StreamId, ws: warp::ws::Ws, clients: Clients, codec_descriptions: CodecDescriptions| {
            ws.on_upgrade(move |socket| handle_publisher(socket, stream_id, clients, codec_descriptions))
        });

    // Route for video consumers
    let consume = warp::path!("consume" / StreamId)
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .and(with_codec_descriptions(codec_descriptions.clone()))
        .map(|stream_id: StreamId, ws: warp::ws::Ws, clients, codec_descriptions: Arc<RwLock<HashMap<String, Vec<u8>>>>| {
            ws.on_upgrade(move |socket| handle_consumer(socket, stream_id, clients, codec_descriptions))
        });
    // Route to serve the player HTML
    // Route to serve the publisher HTML
    let index = warp::path("static").and(warp::filters::fs::dir("static"));
    // Combine routes
    let routes = publish
        .or(consume)
        .or(index)
        .with(warp::cors().allow_any_origin());
    let port = 3043;
    println!("Server started on http://localhost:{}", port);
    warp::serve(routes)
        .tls()
        .cert_path("./cert.crt")
        .key_path("./cert.key")
        .run(([0, 0, 0, 0], port))
        .await;
}

fn with_clients(
    clients: Clients,
) -> impl Filter<Extract = (Clients,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || clients.clone())
}

fn with_codec_descriptions(
    codec_descriptions: CodecDescriptions,
) -> impl Filter<Extract = (CodecDescriptions,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || codec_descriptions.clone())
}

async fn handle_publisher(ws: WebSocket, stream_id: StreamId, clients: Clients, codec_descriptions: Arc<RwLock<HashMap<StreamId, Vec<u8>>>>) {
    println!("New publisher connected for stream: {}", stream_id);

    let (_ws_tx, mut ws_rx) = ws.split();
    let mut first = true;
    let mut counter = 0;
    let mut instant = std::time::Instant::now();
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                if first {
                    first = false;
                    // let extradata = create_hvcc_box(msg.as_bytes()); //do this when receiving annexb
                    let extradata = msg.as_bytes().to_vec(); //do this when receiving hvcc
                    println!("Codec description: {:?}", extradata);
                    let mut codec_descriptions_write = codec_descriptions.write().await;
                    codec_descriptions_write.insert(stream_id.clone(), extradata);
                }
                counter += msg.as_bytes().len();
                if instant.elapsed().as_secs() >= 1 {
                    println!("{} bytes/s", counter);
                    counter = 0;
                    instant = std::time::Instant::now();
                }
                let clients_read = clients.read().await;
                if let Some(stream_clients) = clients_read.get(&stream_id) {
                    // let msg = Message::binary(convert_annexb_to_length_prefixed(msg.as_bytes(), true)); //do this when receiving annexb
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

async fn handle_consumer(ws: WebSocket, stream_id: StreamId, clients: Clients, codec_descriptions: CodecDescriptions) {
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
    let stream_id_clone = stream_id.clone();
    // Send video chunks to the client
    let outgoing = tokio::spawn(async move {
        let codec_descriptions = {
            let codec_descriptions_read = codec_descriptions.read().await;
            codec_descriptions_read.get(&stream_id_clone).cloned().unwrap()
        };

        if let Err(e) = ws_tx.send(Message::binary(codec_descriptions)).await {
            eprintln!("Error sending codec description to consumer: {:?}", e);
            return;
        }
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
