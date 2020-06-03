mod client_handler;
mod crypto;
mod network_worker;

use log::{error, info, trace, warn};

use std::fmt::Formatter;
use std::sync::mpsc::{channel, Receiver, SendError, Sender};
use std::{fmt, thread};
use uuid::Uuid;

const MAX_CLIENTS_THREAD: u8 = 20;
const MAX_MESSAGE_BYTES: u16 = 65535;

#[derive(Clone, PartialEq)]
pub enum MessageOptions {
    NewClient,
    Shutdown,
    None,
}

#[derive(Clone)]
pub struct Message {
    pub(crate) data: String,
    pub client: Uuid,
    pub from_client: bool,
    pub options: MessageOptions,
}

impl Message {
    pub fn new(data: String, client: Uuid, from_client: bool) -> Result<Message, std::io::Error> {
        if data.as_bytes().len() > MAX_MESSAGE_BYTES as usize {
            unimplemented!(
                "Message data is too big!\nMessage bytes {}",
                data.as_bytes().len()
            )
        }
        let message = Message {
            data,
            client,
            from_client,
            options: MessageOptions::None,
        };
        Ok(message)
    }
    pub fn shutdown() -> Message {
        Message {
            data: "".to_string(),
            client: Default::default(),
            options: MessageOptions::Shutdown,
            from_client: false,
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.from_client {
            write!(
                f,
                "Recieved message from {}    Contents: {}",
                self.client.to_string(),
                self.data
            )
        } else {
            write!(
                f,
                "Sending message to: {}    Contents: {}",
                self.client.to_string(),
                self.data
            )
        }
    }
}

/// Initiation struct for network worker and holding all publicly accessible variables
pub struct Network {
    connections: Vec<Uuid>,
    messages_in: Receiver<Message>,
    messages_out: Sender<Message>,
    //TODO Change from string representation
    clients_in: Receiver<Uuid>,
}

impl Network {
    /// Function that starts IO and Listening thread
    ///
    ///
    /// # Arguments
    ///     client_addr: To send new client information to the user thread
    ///     messages_in: New messages to be sent to clients
    ///     messages_out: To send incoming messages to the user thread
    ///
    pub fn init(address: String) -> Network {
        info!("Creating listening handler");
        let (messages_in_sender, messages_in_receiver) = channel();
        let (client_in_sender, client_in_receiver) = channel();
        let (messages_out_sender, messages_out_receiver) = channel();
        //Start listening server
        thread::Builder::new()
            .name(String::from(format!("Listening Server - {}", &address)))
            .spawn(move || {
                let mut worker =
                    network_worker::NetworkWorker::init(client_in_sender, messages_in_sender);
                worker.start(address, messages_out_receiver);
            })
            .expect("Failed to start network listener thread");

        //Return network instance (With client sender for initiated connections)
        Network {
            connections: Vec::new(),
            messages_in: messages_in_receiver,
            messages_out: messages_out_sender,
            clients_in: client_in_receiver,
        }
    }
    pub fn update_clients(&mut self) {
        for client in self.clients_in.try_iter() {
            self.connections.push(client);
        }
    }

    pub fn list_clients(&mut self) -> Vec<Uuid> {
        self.connections.clone()
    }
    pub fn send_message(&mut self, msg: Message) -> Result<(), SendError<Message>> {
        self.messages_out.send(msg)
    }

    pub fn shutdown(&mut self) -> Result<(), SendError<Message>> {
        self.messages_out.send(Message::shutdown())
    }
    pub fn get_messages(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        for msg in self.messages_in.try_iter() {
            if msg.options == MessageOptions::NewClient {
                self.connections.push(msg.data.parse().unwrap());
            } else {
                messages.push(msg);
            }
        }
        messages
    }
    pub fn get_messages_blocking(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        for msg in self.messages_in.iter() {
            if msg.options == MessageOptions::NewClient {
                self.connections.push(msg.data.parse().unwrap());
            } else {
                messages.push(msg);
            }
        }
        messages
    }
}

fn main() {}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
