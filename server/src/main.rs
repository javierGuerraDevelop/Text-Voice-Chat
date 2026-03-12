mod db;
mod networking;
mod state;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

type Tx = mpsc::UnboundedSender<String>;
type PeerMap = Arc<RwLock<HashMap<SocketAddr, Tx>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bind TCP listener to local port
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    // Initialize our shared state map with thread safety
    let peers: PeerMap = Arc::new(RwLock::new(HashMap::new()));

    println!("Server listening on 127.0.0.1:8080");

    loop {
        // Wait for new TCP handshake
        let (stream, addr) = listener.accept().await?;

        // We do this because each background task needs its own "handle" to the map.
        let peers_inner = peers.clone();

        // Tokio thread to handle connection and give handle_connection ownership with move
        tokio::spawn(async move {
            handle_connection(peers_inner, stream, addr).await;
        });
    }
}

/**
 * handle_connection: for an individual client.
 * @peers: The shared map of all connected users.
 * @stream: The TCP socket for this client.
 * @addr: The IP/Port of this client (used as a unique ID/Key).
 */
async fn handle_connection(peers: PeerMap, stream: TcpStream, addr: SocketAddr) {
    println!("New connection from: {addr}");

    // Pipe for messages. tx->sender, rx->receiever
    let (tx, _rx) = mpsc::unbounded_channel();

    // Add address to the global peermap
    peers.write().unwrap().insert(addr, tx);

    println!("Client {addr} registered in the PeerMap");
}
