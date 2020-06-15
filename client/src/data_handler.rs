use std::fmt::Formatter;
use std::io::Cursor;
use std::path::Path;
use std::str::FromStr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;
use std::{fmt, io, thread};

use log::{error, info, trace, warn};
use rustls::ClientConfig;
use std::net::Shutdown::Read;
use strum::EnumMessage;
use strum::IntoEnumIterator;
use strum_macros::AsRefStr;
use strum_macros::EnumIter;
use strum_macros::EnumMessage;
use strum_macros::EnumString;
use text_io::read;

pub mod data_handler;

use network_listener::protos::message::Request;

#[derive(EnumIter, EnumString, EnumMessage, Debug, AsRefStr)]
enum Commands {
    #[strum(
        message = "Update",
        detailed_message = "This retrieves any new messages and connections"
    )]
    Update,
    #[strum(
        message = "List",
        detailed_message = "This displays all current connections"
    )]
    List,
    #[strum(message = "message", detailed_message = "This sends a message")]
    Message,
    #[strum(message = "Exit", detailed_message = "This exits the program")]
    Exit,
}

impl Commands {
    fn get_help_dialog() -> String {
        let mut out = String::new();
        out.push_str("Possible commands:\n");
        for command in Commands::iter() {
            out.push_str(
                format!(
                    "      {} - {}\n",
                    command.get_message().unwrap_or(command.as_ref()),
                    command
                        .get_detailed_message()
                        .unwrap_or("No Help message :(")
                )
                .as_ref(),
            );
        }
        return out;
    }
    fn get_user_command() -> Commands {
        println!("\n\nEnter command: ");
        let input: String = read!("{}\n");
        match Commands::from_str(&input) {
            Ok(t) => t,
            Err(_) => {
                println!("Invalid option!\n\n{}", Commands::get_help_dialog());
                Commands::get_user_command()
            }
        }
    }
}

struct ClientUser {
    addr: String,
    messages: Vec<Request>,
    nickname: String,
}

impl fmt::Display for ClientUser {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Client with address {}, and nickname {}",
            self.addr, self.nickname
        )
    }
}

pub struct InputLoop {
    user_client_receiver: Receiver<String>,
    messages_in_receiver: Receiver<Request>,
    messages_out_sender: Sender<Request>,
    clients: Vec<ClientUser>,
    //    network_struct: Network,
}

impl InputLoop {
    /*
    pub fn new(address: String) -> InputLoop {
        let (user_client_sender, user_client_receiver) = channel();
        let (messages_in_sender, messages_in_receiver) = channel();
        let (messages_out_sender, messages_out_receiver) = channel();
        let network_struct = network::Network::init(
            address,
            user_client_sender,
            messages_in_sender,
            messages_out_receiver,
        );
        InputLoop {
            user_client_receiver,
            messages_in_receiver,
            messages_out_sender,
            clients: Vec::new(),
            network_struct,
        }

    */

    ///The main user input loop
    /// Responds to user input
    pub fn start(&mut self) {
        info!("Started user input loop");
        loop {
            match Commands::get_user_command() {
                Commands::Message => {
                    println!("Enter the name of client to message: ");
                    let name: String = read!("{}\n");
                    println!("Enter the message to send: ");
                    let msg: String = read!("{}\n");

                    if self.send_message(name, msg) {
                        println!("Sent message");
                    } else {
                        println!("Failed to find recipient");
                    }
                }
                Commands::Update => {
                    self.update_clients();
                    self.check_messages();
                }
                Commands::List => {
                    println!("Current connections: ");
                    for client in &self.clients {
                        println!("      {}", &client);
                    }
                }
                Commands::Exit => {
                    println!("Goodbye!");
                    return;
                }
            }
        }
    }
    fn check_messages(&mut self) {
        info!("Updating messages");
        for msg in self.messages_in_receiver.try_iter() {
            for client in &mut self.clients {
                if client.addr == msg.client_id.to_string() {
                    println!("{:?}", msg);
                    client.messages.push(msg.clone());
                }
            }
        }
    }
    fn check_messages_bench(&mut self) -> Vec<i64> {
        info!("Updating messages");
        let mut differences = Vec::new();
        for msg in self.messages_in_receiver.try_iter() {
            for client in &mut self.clients {
                if client.addr == msg.client_id.to_string() {
                    let time: Result<i64, <i64 as FromStr>::Err> = msg.data.get(0).unwrap().parse();
                    match time {
                        Ok(t) => {
                            let rec_time: i64 = chrono::Utc::now().timestamp_millis();
                            println!("Time {} dif {}", rec_time, t);
                            println!("Received {}", rec_time - t);
                            differences.push(rec_time - t);
                        }
                        Err(_) => {
                            for fuck in msg.data.get(0).unwrap().split_whitespace() {
                                let time: Result<i64, <i64 as FromStr>::Err> =
                                    fuck[0..fuck.len()].parse();
                                if time.is_ok() {
                                    let current = chrono::Utc::now().timestamp_millis();
                                    let received = time.unwrap();
                                    //println!("CUrrent {} Time Sent{}",current,receievd);
                                    let different = current - received;
                                    differences.push(different);
                                }
                            }
                        }
                    }
                }
            }
        }
        differences
    }

    fn update_clients(&mut self) {
        trace!("Checking for new clients");
        for client in self.user_client_receiver.try_iter() {
            //println!("New client from {}", client);

            self.clients.push(ClientUser {
                addr: client,
                messages: vec![],
                nickname: "".to_string(),
            })
        }
    }

    fn send_message(&mut self, client_name: String, msg_data_str: String) -> bool {
        println!("Attempting to send message to {}", client_name);
        for client in &mut self.clients {
            if client.addr == client_name {
                trace!("Making message request to {}", &client);
                return match Request::new_from_fields(
                    Vec::new(),
                    Request::new_secure_uuid_v4(),
                    false,
                ) {
                    Ok(msg) => {
                        if self.messages_out_sender.send(msg).is_err() {
                            error!("Failed to send message to networking thread");
                            false
                        } else {
                            true
                        }
                    }
                    Err(_) => {
                        println!("message data is too big!");
                        false
                    }
                };
            }
        }
        false
    }

    pub fn shutdown(&mut self) {
        self.messages_out_sender
            .send(Request::shutdown())
            .expect("Failed to send shutdown command");
    }
}
