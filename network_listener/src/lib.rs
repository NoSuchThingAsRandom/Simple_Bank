extern crate structs;
mod client_handler;
mod crypto;
mod network_worker;
mod server_connection;

use structs::protos::message::{Request, Request_RequestType};

use std::sync::mpsc::{channel, Receiver, SendError, Sender};
use std::thread;

use log::{info, warn};

use uuid::Uuid;

pub const ADDRESS: &str = "127.0.0.1:";
pub const DATA_MISC_PORT: &str = "49700";
pub const DATA_TRANSACTIONS_PORT: &str = "49800";
pub const DATA_ACCOUNTS_PORT: &str = "49900";
pub const LOAD_BALANCER_PORT: &str = "50000";

const MAX_CLIENTS_THREAD: u8 = 20;
const _MAX_MESSAGE_BYTES: u16 = 65535;

pub struct Client {
    pub addr: String,
    pub messages: Vec<Request>,
    messages_in: Receiver<Request>,
    messages_out: Sender<Request>,
}

impl Client {
    pub fn start(addr: String) -> Client {
        let (incoming_messages_out, incoming_messages_in) = channel();
        let (outgoing_messages_out, outgoing_messages_in) = channel();
        let server_address = addr.clone();
        thread::spawn(move || {
            let mut conn = server_connection::ServerConn::new(addr.clone());
            conn.start(incoming_messages_out, outgoing_messages_in)
                .unwrap();
        });
        Client {
            addr: server_address,
            messages: Vec::new(),
            messages_in: incoming_messages_in,
            messages_out: outgoing_messages_out,
        }
    }
    pub fn get_all_messages(&mut self) -> Vec<Request> {
        self.messages_in.try_iter().collect()
    }

    pub fn get_all_message_blocking(&mut self) -> Vec<Request> {
        self.messages_in.iter().collect()
    }
    pub fn get_singular_message(&mut self) -> Request {
        self.messages_in.recv().unwrap()
    }
    pub fn send_message(&mut self, message: Request) -> Result<(), SendError<Request>> {
        self.messages_out.send(message)
    }
}

/// Initiation struct for network worker and holding all publicly accessible variables
pub struct Server {
    pub connections: Vec<Uuid>,
    messages_in: Receiver<Request>,
    messages_out: Sender<Request>,
    //TODO Change from string representation
    clients_in: Receiver<Uuid>,
}

impl Server {
    /// Function that starts IO and Listening thread
    ///
    ///
    /// # Arguments
    ///     client_addr: To send new client information to the user thread
    ///     messages_in: New messages to be sent to clients
    ///     messages_out: To send incoming messages to the user thread
    ///
    pub fn init(address: String) -> Server {
        //TODO Create client authentication for backend servers and load_balancers
        info!("Creating listening handler");
        let (messages_in_sender, messages_in_receiver) = channel();
        let (client_in_sender, client_in_receiver) = channel();
        let (messages_out_sender, messages_out_receiver) = channel();
        //Start listening load_balancer
        thread::Builder::new()
            .name(String::from(format!("Listening Server - {}", &address)))
            .spawn(move || {
                let mut worker =
                    network_worker::NetworkWorker::init(client_in_sender, messages_in_sender);
                worker.start(address, messages_out_receiver);
            })
            .expect("Failed to start network listener thread");

        //Return network instance (With client sender for initiated connections)
        Server {
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
    pub fn send_message(&mut self, msg: Request) -> Result<(), SendError<Request>> {
        self.messages_out.send(msg)
    }

    pub fn shutdown(&mut self) -> Result<(), SendError<Request>> {
        self.messages_out.send(Request::shutdown())
    }
    pub fn get_messages(&mut self) -> Vec<Request> {
        let messages: Vec<Request> = self.messages_in.try_iter().collect();
        for msg in &messages {
            //TODO Check if it is a new client
            warn!("Need to check for new client connection");
            if msg.field_type == Request_RequestType::AUTHENTICATE {
                self.connections.push(msg.client_id.parse().unwrap());
            }
        }
        messages
    }
    pub fn get_messages_blocking(&mut self) -> Vec<Request> {
        let messages: Vec<Request> = self.messages_in.iter().collect();
        for msg in &messages {
            //TODO Check if it is a new client
            warn!("Need to check for new client connection");
            if msg.field_type == Request_RequestType::AUTHENTICATE {
                self.connections.push(msg.client_id.parse().unwrap());
            }
        }
        messages
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
