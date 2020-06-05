mod client_handler;
mod crypto;
pub mod messages_request;
mod network_worker;
mod server_connection;

use log::{error, info, trace, warn};

use crate::messages_request::asd::request::{
    AccountType, AuthenticateType, DetailedType, MiscType, RequestType, TransactionType,
};
use crate::messages_request::asd::Request;
use rand::{RngCore, SeedableRng};
use std::fmt::Formatter;
use std::net::Shutdown::Read;
use std::sync::mpsc::{channel, Receiver, SendError, Sender};
use std::{fmt, thread};
use uuid::{Builder, Uuid, Variant, Version};

pub const ADDRESS: &str = "127.0.0.1";
pub const DATA_MISC_PORT: &str = "49700";
pub const DATA_TRANSACTIONS_PORT: &str = "49800";
pub const DATA_ACCOUNTS_PORT: &str = "49900";
pub const LOAD_BALANCER_PORT: &str = "50000";

const MAX_CLIENTS_THREAD: u8 = 20;
const MAX_MESSAGE_BYTES: u16 = 65535;

impl Request {
    pub fn new(data: String, client: Uuid, from_client: bool) -> Result<Request, std::io::Error> {
        if data.as_bytes().len() > MAX_MESSAGE_BYTES as usize {
            unimplemented!(
                "message data is too big!\nmessage bytes {}",
                data.as_bytes().len()
            )
        }
        let message = Request {
            r#type: RequestType::Misc as i32,
            user_id: "".to_string(),
            client_id: client.to_string(),
            data: vec![data],
            from_client,
            detailed_type: Option::from(messages_request::asd::request::DetailedType::Misc(1)),
        };
        Ok(message)
    }
    pub fn shutdown() -> Request {
        Request {
            r#type: RequestType::Shutdown as i32,
            user_id: "".to_string(),
            client_id: "".to_string(),
            data: Vec::new(),
            from_client: false,
            detailed_type: None,
        }
    }
    pub fn new_secure_uuid_v4() -> Uuid {
        let mut rng = rand::rngs::StdRng::from_entropy();
        let mut bytes = [0; 16];

        rng.fill_bytes(&mut bytes);

        Builder::from_bytes(bytes)
            .set_variant(Variant::RFC4122)
            .set_version(Version::Random)
            .build()
    }
}

/*
#[derive(Clone, PartialEq)]
pub enum MessageOptions {
    NewClient,
    Shutdown,
    None,
}

#[derive(Clone)]
pub struct message {
    pub data: String,
    pub client: Uuid,
    pub from_client: bool,
    pub options: MessageOptions,
}*/

/*
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
*/

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
        let mut messages = Vec::new();
        for msg in self.messages_in.try_iter() {
            //TODO Check if it is a new client
            warn!("Checking for client, using incorrect methiods");
            if msg.r#type == RequestType::Authenticate as i32 {
                //if msg.detailed_type == MessageOptions::NewClient {
                self.connections.push(msg.client_id.parse().unwrap());
            } else {
                messages.push(msg);
            }
        }
        messages
    }
    pub fn get_messages_blocking(&mut self) -> Vec<Request> {
        let mut messages = Vec::new();
        for msg in self.messages_in.iter() {
            //TODO Check if it is a new client
            warn!("Checking for client, using incorrect methiods");
            if msg.r#type == RequestType::Authenticate as i32 {
                self.connections.push(msg.client_id.parse().unwrap());
            } else {
                messages.push(msg);
            }
        }
        messages
    }
}

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
            conn.start(incoming_messages_out, outgoing_messages_in);
        });
        Client {
            addr: server_address,
            messages: Vec::new(),
            messages_in: incoming_messages_in,
            messages_out: outgoing_messages_out,
        }
    }
    pub fn get_messages(&mut self) -> Vec<Request> {
        self.messages_in.try_iter().collect()
    }

    pub fn get_message_blocking(&mut self) -> Vec<Request> {
        self.messages_in.iter().collect()
    }
    pub fn send_message(&mut self, message: Request) -> Result<(), SendError<Request>> {
        self.messages_out.send(message)
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
