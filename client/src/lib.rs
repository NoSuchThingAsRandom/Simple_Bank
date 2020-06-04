extern crate strum;
extern crate strum_macros;

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
use strum::EnumMessage;
use strum::IntoEnumIterator;
use strum_macros::AsRefStr;
use strum_macros::EnumIter;
use strum_macros::EnumMessage;
use strum_macros::EnumString;
use text_io::read;

use crate::network::{ServerConn, MAX_MESSAGE_BYTES};

pub mod network;

impl Message {
    pub fn new(data: String) -> Result<Message, std::io::Error> {
        if data.as_bytes().len() > MAX_MESSAGE_BYTES as usize {
            unimplemented!(
                "Message data is too big!\nMessage bytes {}",
                data.as_bytes().len()
            )
        }
        let message = Message {
            data,
            sender: String::new(),
            recipient: String::new(),
            options: MessageOptions::None,
        };
        Ok(message)
    }
    pub fn shutdown() -> Message {
        Message {
            data: "".to_string(),
            sender: "".to_string(),
            recipient: "".to_string(),
            options: MessageOptions::Shutdown,
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Message from: {}    To: {}    Contents: {}",
            self.sender, self.recipient, self.data
        )
    }
}

#[derive(EnumIter, EnumString, EnumMessage, Debug, AsRefStr)]
enum Commands {
    #[strum(
        message = "Update",
        detailed_message = "This retrieves any new messages and connections"
    )]
    Update,

    #[strum(message = "Message", detailed_message = "This sends a message")]
    Message,

    Test,

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
                        .unwrap_or("No Help Message :(")
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
    messages: Vec<Message>,
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
    messages_in_receiver: Receiver<Message>,
    messages_out_sender: Sender<Message>,
    archive_message: Vec<Message>,
}

impl InputLoop {
    pub fn new(address: String) -> InputLoop {
        let (messages_in_sender, messages_in_receiver) = channel();
        let (messages_out_sender, messages_out_receiver) = channel();
        thread::spawn(move || {
            let mut network = network::ServerConn::new(address);
            network.start(messages_in_sender, messages_out_receiver);
        });
        InputLoop {
            messages_in_receiver,
            messages_out_sender,
            archive_message: Vec::new(),
        }
    }
    ///The main user input loop
    /// Responds to user input
    pub fn start(&mut self) {
        info!("Started user input loop");
        loop {
            match Commands::get_user_command() {
                Commands::Message => {
                    println!("Enter the message to send: ");
                    let msg: String = read!("{}\n");

                    if self.send_message(msg) {
                        println!("Sent message");
                    } else {
                        println!("Failed to send message");
                    }
                }
                Commands::Update => {
                    self.check_messages();
                }
                Commands::Exit => {
                    println!("Goodbye!");
                    return;
                }
                Commands::Test => println!("Should execute some tests..."),
            }
        }
    }
    fn check_messages(&mut self) {
        info!("Updating messages");
        for msg in self.messages_in_receiver.try_iter() {
            println!("{}", msg);
            self.archive_message.push(msg);
        }
    }

    fn send_message(&mut self, msg_data: String) -> bool {
        return match Message::new(msg_data) {
            Ok(msg) => {
                info!("Attempting to send message {}", msg);
                if let Err(e) = self.messages_out_sender.send(msg) {
                    error!("Failed to send message to networking thread ({})", e);
                    false
                } else {
                    true
                }
            }
            Err(_) => {
                println!("Message is too big!");
                false
            }
        };
    }

    pub fn shutdown(&mut self) {
        self.messages_out_sender
            .send(Message::shutdown())
            .expect("Failed to send shutdown command");
    }
}
