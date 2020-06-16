mod client_handler;
mod crypto;
mod network_worker;
pub mod protos;
mod server_connection;

use crate::protos::message::{
    Request, Request_RequestType, Request_ResultType, Request_oneof_detailed_type,
};

use std::sync::mpsc::{channel, Receiver, SendError, Sender};
use std::thread;

use log::{error, info, warn};
use rand::{RngCore, SeedableRng};
use uuid::{Builder, Uuid, Variant, Version};

pub const ADDRESS: &str = "127.0.0.1:";
pub const DATA_MISC_PORT: &str = "49700";
pub const DATA_TRANSACTIONS_PORT: &str = "49800";
pub const DATA_ACCOUNTS_PORT: &str = "49900";
pub const LOAD_BALANCER_PORT: &str = "50000";

const MAX_CLIENTS_THREAD: u8 = 20;
const _MAX_MESSAGE_BYTES: u16 = 65535;

/**
    Client uuid - An identifier for the socket connection
    User uuid   - An identifier for a authenticated user

**/
impl Request {
    pub fn new_from_fields(
        data: Vec<String>,
        client: Uuid,
        from_client: bool,
    ) -> Result<Request, std::io::Error> {
        let message = Request {
            field_type: Default::default(),
            user_id: Default::default(),
            client_id: client.to_string(),
            data: protobuf::RepeatedField::from_vec(data),
            from_client,
            detailed_type: Default::default(),
            token_id: "".to_string(),
            unknown_fields: protobuf::UnknownFields::new(),
            cached_size: Default::default(),
        };
        Ok(message)
    }
    pub fn success_result(
        data: Vec<String>,
        client: String,
        result_type: Request_ResultType,
    ) -> Result<Request, std::io::Error> {
        let message = Request {
            field_type: Request_RequestType::Result,
            user_id: Default::default(),
            client_id: client,
            data: protobuf::RepeatedField::from_vec(data),
            from_client: false,
            detailed_type: Some(Request_oneof_detailed_type::result(result_type)),
            token_id: "".to_string(),
            unknown_fields: protobuf::UnknownFields::new(),
            cached_size: Default::default(),
        };
        Ok(message)
    }
    pub fn shutdown() -> Request {
        Request {
            field_type: Request_RequestType::SHUTDOWN,
            user_id: "".to_string(),
            client_id: "".to_string(),
            data: Default::default(),
            from_client: false,
            detailed_type: None,
            token_id: "".to_string(),
            unknown_fields: Default::default(),
            cached_size: Default::default(),
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
            //TODO Need to renable client network thread
            error!("Need to renable network thread!");

            //let mut conn = server_connection::ServerConn::new(addr.clone());
            //conn.start(incoming_messages_out, outgoing_messages_in).unwrap();
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
