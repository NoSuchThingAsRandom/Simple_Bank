use crate::crypto::{TlsConnection, TlsServer};
//use crate::{Message, MessageOptions};

use std::collections::HashMap;
use std::sync::mpsc::*;

use crate::messages_request::asd::request::RequestType;
use crate::messages_request::asd::Request;
use log::{error, info, trace, warn};
use mio::{Events, Interest, Poll, Token};
use rand::{RngCore, SeedableRng};
use std::time::Duration;
use uuid::{Builder, Uuid, Variant, Version};

//pub(crate) const ADDR: &str = "127.0.0.1:5962";

//const SERVER: Token = Token(11);

const BUFFER_SIZE: usize = 512;

/// Struct representing a singular client
pub struct Client {
    pub addr: String,
    pub uuid: Uuid,
    client_buffer: Vec<u8>,
    tls: TlsConnection,
}

impl Client {
    pub fn new(addr: String, tls: TlsConnection) -> Client {
        Client {
            addr,
            uuid: Request::new_secure_uuid_v4(),
            client_buffer: Vec::new(),
            tls,
        }
    }

    ///Reads group of messages from a client
    fn read_data(&mut self) -> Result<Vec<Request>, Box<dyn std::error::Error>> {
        trace!("Reading data from socket {}", self.addr);
        let mut buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
        return match self.tls.read_plaintext(&mut buffer) {
            Ok(n) => {
                if n == BUFFER_SIZE {
                    info!("Buffer has been filled!");
                    self.client_buffer.append(&mut buffer.to_vec());
                    self.read_data()
                } else if n > 0 {
                    info!("Read {} bytes", n);
                    self.client_buffer.append(&mut buffer[0..n].to_vec());
                    Ok(self.get_messages_from_buffer())
                } else {
                    warn!("Didn't read data, closing!");
                    Ok(self.get_messages_from_buffer())
                }
            }
            Err(e) => {
                error!("Error ({}) encountered reading from {}", e, self.addr);
                Err(e)
            }
        };
    }

    ///Attempts to export messages from internal buffer
    pub fn get_messages_from_buffer(&mut self) -> Vec<Request> {
        trace!("Getting messages from buffer");
        let mut messages = Vec::new();
        while self.client_buffer.len() > 2 {
            let cloned_buffer = self.client_buffer.clone();
            let (size, buffer) = cloned_buffer.split_at(2);
            let data_size = u16::from_be_bytes([size[0], size[1]]);
            trace!(
                "message size is {}, buffer size {}",
                data_size,
                buffer.len()
            );
            if buffer.len() >= data_size as usize {
                let (msg_bytes, remaining_bytes) = buffer.split_at(data_size as usize).clone();

                let msg = Request::new(
                    String::from_utf8(msg_bytes.to_vec()).expect("Invalid utf-8 received"),
                    self.uuid,
                    true,
                )
                .unwrap();
                trace!("Received {}", msg);
                messages.push(msg);
                self.client_buffer = remaining_bytes.to_vec();
            } else {
                break;
            }
        }
        trace!("Finished retrieving messages from buffer");
        return messages;
    }
}

/// Struct representing a mio thread poll for multiple clients
pub struct ClientIo {
    clients: HashMap<Token, Client>,
    current_client_count: usize,
    poll: Poll,
    incoming_clients: Receiver<Client>,
    messages_in: Sender<Request>,
    messages_out: Receiver<Request>,
}

impl ClientIo {
    pub fn new(
        incoming_clients: Receiver<Client>,
        messages_in: Sender<Request>,
        messages_out: Receiver<Request>,
    ) -> ClientIo {
        ClientIo {
            clients: HashMap::new(),
            current_client_count: 0,
            poll: Poll::new().unwrap(),
            incoming_clients,
            messages_in,
            messages_out,
        }
    }

    ///Mio poll for sending messages, receiving messages and adding clients
    pub fn start(&mut self) -> Result<(), TryRecvError> {
        let mut events = Events::with_capacity(128);
        info!("Starting IO writer loop");
        let mut shutdown = false;
        while !shutdown {
            //Check for new clients
            if self.check_clients().is_err() {
                shutdown = true;
            }
            for client in self.clients.values_mut() {
                client.tls.check_write();
            }
            //Send messages
            let mut messages: Vec<Request> = self.messages_out.try_iter().collect();
            for client in self.clients.values_mut() {
                messages.retain(|msg| {
                    if msg.r#type == RequestType::Shutdown as i32 {
                        shutdown = true;
                        false
                    } else {
                        if client.uuid == msg.client_id.parse().unwrap() {
                            match client.tls.write_message(msg) {
                                Ok(_) => false,
                                Err(e) => {
                                    error!("Failed to send message ({}), ({})", msg, e);
                                    true
                                }
                            }
                        } else {
                            true
                        }
                    }
                });
            }
            for msg in messages {
                warn!("Failed to send {}", msg);
            }

            //Check for incoming messages
            self.poll
                .poll(&mut events, Some(Duration::from_millis(100)))
                .expect("Failed to poll for new events");
            for event in events.iter() {
                info!("Received event for {:?}", event);

                if event.is_readable() {
                    let socket = self.clients.get_mut(&event.token()).unwrap();
                    match socket.read_data() {
                        Ok(messages) => {
                            for msg in messages {
                                match self.messages_in.send(msg) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("Send message to user thread failed {}", e);
                                        shutdown = true;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to read data {}", e);
                            self.clients.remove(&event.token());
                            self.current_client_count -= 1;
                        }
                    }
                } else {
                    warn!("Non readable event {:?}", event);
                }
            }
        }
        info!("Starting shutdown of IO thread");
        trace!("Checking for unparsed data in buffers");
        for client in self.clients.values_mut() {
            if !client.client_buffer.is_empty() {
                info!("Client buffer for {} is not empty!!!", client.addr);
                trace!("Dumping buffer\n{:?}", client.client_buffer);

                let messages = client.read_data();
                if messages.is_ok() {
                    for msg in messages.unwrap() {
                        println!("Got {}", msg);
                        match self.messages_in.send(msg) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("Send message to user thread failed {}", e);
                            }
                        }
                    }
                } else {
                    error!(
                        "Failed getting data from buffer {}",
                        messages.err().unwrap()
                    );
                }
            }
            match client.tls.shutdown() {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to close {}, as ({})", client.addr, e);
                }
            };
        }
        info!("Closed IO thread");
        Ok(())
    }

    /// Function for adding new clients to mio poll
    fn check_clients(&mut self) -> Result<(), TryRecvError> {
        let mut new_clients = true;
        while new_clients {
            match self.incoming_clients.try_recv() {
                Ok(mut c) => {
                    trace!("Added new client to writer {}", c.addr);
                    let token = Token(self.current_client_count);
                    self.current_client_count += 1;
                    self.poll
                        .registry()
                        .register(&mut c.tls.socket, token, Interest::READABLE)
                        .unwrap();
                    self.clients.insert(token, c);
                }
                Err(e) => {
                    if e == TryRecvError::Empty {
                        new_clients = false;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }
}
