use connection::Connection;
use std::collections::HashMap;
use std::str;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, Receiver, Sender};

mod connection;

struct User {
    _name: String,
    _connected: bool,
    channel: Sender<Command>,
}

#[derive(Debug)]
enum Command {
    SetName(String, Sender<Command>),
    Message(String, String),
    Announcement(String),
    Disconnect(String),
}

pub async fn run() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on port 8080");

    let (tx, rx): (Sender<Command>, Receiver<Command>) = mpsc::channel(1000);

    tokio::spawn(async move {
        process_manager(rx).await;
    });

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        //println!("Starting connection");
        let conn = Connection::new(socket);
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            process_client(conn, tx_clone).await;
        });
    }
}

async fn process_manager(mut rx: Receiver<Command>) {
    let mut users: HashMap<String, User> = HashMap::new();

    while let Some(command) = rx.recv().await {
        match command {
            Command::SetName(name, tx) => {
                if users.contains_key(&name) {
                    let disconnect_msg = format!("{} already exists", name);
                    let _ = tx.send(Command::Disconnect(disconnect_msg)).await;
                } else {
                    users.insert(
                        name.clone(),
                        User {
                            _name: name.clone(),
                            _connected: true,
                            channel: tx.clone(),
                        },
                    );

                    let mut contained_users: Vec<String> =
                        users.keys().filter(|key| key != &&name).cloned().collect();
                    contained_users.sort();
                    let result = contained_users.join(", ");

                    let contain_msg = format!("* The room contains: {}\n", result);
                    //println!("sending: {}", contain_msg);
                    let _ = tx.send(Command::Announcement(contain_msg)).await;

                    let announce = format!("* {} has entered the room\n", name);
                    for (key, user) in &users {
                        if *name != key.clone() {
                            let _ = user
                                .channel
                                .send(Command::Announcement(announce.clone()))
                                .await;
                        }
                    }
                }
            }
            Command::Message(from, msg) => {
                for (name, user) in &users {
                    if *name != from.clone() {
                        let _ = user
                            .channel
                            .send(Command::Message(from.clone(), msg.clone()))
                            .await;
                    }
                }
            }
            Command::Disconnect(from) => {
                users.remove(&from);
                let announce = format!("* {} has left the room\n", from);
                for (key, user) in &users {
                    if *from != key.clone() {
                        let _ = user
                            .channel
                            .send(Command::Announcement(announce.clone()))
                            .await;
                    }
                }
            }
            Command::Announcement(_) => println!("invalid command"),
        }
    }
}

fn strip_trailing_newline(input: &str) -> &str {
    input
        .strip_suffix("\r\n")
        .or(input.strip_suffix("\n"))
        .unwrap_or(input)
}

fn validate_username(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|x| x.is_ascii_alphanumeric())
}

async fn process_client(mut connection: Connection, manager_tx: Sender<Command>) {
    connection
        .write_message("Welcome to budgetchat! What shall I call you?\n")
        .await;
    let response = connection.read_message().await;
    //println!("Response: {:?}", response);
    let mut name = str::from_utf8(&response).unwrap().to_string();
    name = strip_trailing_newline(&name).to_string();

    //println!("validating name: {}", name);
    if !validate_username(&name) {
        //println!("Disconnected");
        return;
    }

    let (tx, mut rx): (Sender<Command>, Receiver<Command>) = mpsc::channel(1000);

    manager_tx
        .send(Command::SetName(name.clone(), tx))
        .await
        .unwrap();

    loop {
        tokio::select! {
            Some(command) = rx.recv() => {
                match command {
                    Command::SetName(_, _) => println!("Invalid command"),
                    Command::Message(from, msg) => {
                        connection.write_message(&format!("[{}] {}", from, msg)).await;
                    }
                    Command::Announcement(msg) => connection.write_message(&msg).await,
                    Command::Disconnect(msg) => {
                        connection.write_message(&msg).await;
                        //println!("User disconnected");
                        break;
                    }
                }
            }
            response = connection.read_message() => {
                let msg = str::from_utf8(&response).unwrap().to_string();
                if response.is_empty() {
                    let _ = manager_tx.send(Command::Disconnect(name.clone())).await;
                    break;
                }
                let _ = manager_tx.send(Command::Message(name.clone(), msg)).await;
            }

        }
    }
}
