use log::debug;
use std::collections::HashMap;
use tokio::sync::mpsc::{Receiver, Sender};

struct User {
    name: String,
    channel: Sender<ClientCommand>,
}

pub enum RoomCommand {
    Join(String, Sender<ClientCommand>),
    Message(String, String),
    Disconnect(String),
}

pub enum ClientCommand {
    Message(String),
    Disconnect,
}

fn validate_username(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|x| x.is_ascii_alphanumeric())
}

async fn send_to_others(users: &HashMap<String, User>, from: String, msg: String) {
    for user in users.values() {
        if from != user.name {
            debug!("(server) -> ({}): {}", user.name, msg);
            let _ = user.channel.send(ClientCommand::Message(msg.clone())).await;
        }
    }
}

pub async fn room_manager(mut rx: Receiver<RoomCommand>) {
    let mut users: HashMap<String, User> = HashMap::new();

    while let Some(command) = rx.recv().await {
        match command {
            RoomCommand::Join(name, client_tx) => {
                if validate_username(&name) && !users.contains_key(&name) {
                    let mut contained_users: Vec<String> = users.keys().cloned().collect();
                    contained_users.sort();
                    users.insert(
                        name.clone(),
                        User {
                            name: name.clone(),
                            channel: client_tx.clone(),
                        },
                    );

                    let contain_msg =
                        format!("* The room contains: {}", contained_users.join(", "));
                    debug!("(server) -> ({}): {}", name.clone(), contain_msg);
                    let _ = client_tx.send(ClientCommand::Message(contain_msg)).await;

                    // Announce join to other users
                    let announce = format!("* {} has entered the room", name.clone());
                    send_to_others(&users, name, announce).await;
                } else {
                    debug!("Invalid name: {} Sending disconnect", name);
                    let _ = client_tx.send(ClientCommand::Disconnect).await;
                }
            }
            RoomCommand::Message(name, msg) => {
                let msg = format!("[{}] {}", name, msg);
                send_to_others(&users, name, msg).await;
            }
            RoomCommand::Disconnect(name) => {
                let announce = format!("* {} has left the room", name);
                send_to_others(&users, name, announce).await;
            }
        }
    }
}
