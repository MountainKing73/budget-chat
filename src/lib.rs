use futures::sink::SinkExt;
use log::{debug, error, info};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

mod room;

use crate::room::{ClientCommand, RoomCommand, room_manager};

pub async fn run() {
    env_logger::init();
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("Listening on port 8080");

    let (room_tx, room_rx): (Sender<RoomCommand>, Receiver<RoomCommand>) = mpsc::channel(1000);

    tokio::spawn(async move {
        room_manager(room_rx).await;
    });

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        debug!("Starting connection");
        let room_tx = room_tx.clone();
        tokio::spawn(async move {
            handle_client(socket, room_tx).await;
        });
    }
}

async fn handle_client(mut stream: TcpStream, room_tx: Sender<RoomCommand>) {
    let (read, write) = stream.split();
    let encoder = LinesCodec::new();

    let mut writer = FramedWrite::new(write, encoder);
    let _ = writer
        .send("Welcome to budgetchat! What shall I call you?")
        .await;

    let decoder = LinesCodec::new();
    let mut reader = FramedRead::new(read, decoder);

    let name = reader.next().await.unwrap().unwrap();

    let (client_tx, mut client_rx): (Sender<ClientCommand>, Receiver<ClientCommand>) =
        mpsc::channel(100);
    debug!("({}) -> (server) Join request", name);
    let _ = room_tx
        .send(RoomCommand::Join(name.clone(), client_tx))
        .await;

    loop {
        tokio::select! {
            Some(command) = client_rx.recv() => {
                match command {
                    ClientCommand::Message(msg) => {
                        let _ = writer.send(msg).await;
                    }
                    ClientCommand::Disconnect => {
                        break;
                    }
                }
            }
            result = reader.next() => match result {
                Some(Ok(msg)) => {
                    debug!("({}) -> (server) {}", name, msg);
                    let _ = room_tx.send(RoomCommand::Message(name.clone(),msg)).await;
                }
                Some(Err(e)) => {
                    error!("Error reading message {}", e);
                    break;
                }
                None => {
                    debug!("({}) -> (server) Disconnect", name);
                    let _ = room_tx.send(RoomCommand::Disconnect(name.clone())).await;
                    break;
                }
            }
        }
    }
}
