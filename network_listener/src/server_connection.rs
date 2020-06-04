use crate::{crypto, Message, MessageOptions};
use std::io::Cursor;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use log::{error, info, trace, warn};
use mio::{Events, Interest, Poll, Token};
use rustls::ClientConfig;
use uuid::Uuid;

const BUFFER_SIZE: usize = 512;
/// Struct representing a singular client
pub struct ServerConn {
    pub addr: String,
    pub uuid: Uuid,
    client_buffer: Vec<u8>,
    tls: crypto::TlsConnection,
}

impl ServerConn {
    pub fn new(hostname: String) -> ServerConn {
        //Create client config
        let mut config = ClientConfig::new();
        let ca_file = Path::new("../certs/CA/myCA.pem");
        let file = std::fs::read(ca_file).expect("Failed to read file.");
        let mut pem = Cursor::new(file);
        config
            .root_store
            .add_pem_file(&mut pem)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid cert"))
            .expect("Unable to create configuration object.");

        //Initiate Connection
        let stream = mio::net::TcpStream::connect(hostname.to_string().parse().unwrap()).unwrap();
        info!("Created new connection to {:?}", stream.peer_addr());
        warn!("Need to provide proper dns hostname");
        let dns_hostname = webpki::DNSNameRef::try_from_ascii_str("localhost").unwrap();
        let session = rustls::ClientSession::new(&Arc::new(config), dns_hostname);
        let conn = crypto::TlsConnection::new(stream, Box::from(session));

        ServerConn {
            addr: hostname,
            client_buffer: Vec::new(),
            tls: conn,
            uuid: Message::new_secure_uuid_v4(),
        }
    }
    pub fn start(
        &mut self,
        incoming_messages: Sender<Message>,
        outgoing_message: Receiver<Message>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut poll: mio::Poll = Poll::new().unwrap();
        const READ_MESSAGE: Token = mio::Token(0);
        poll.registry()
            .register(&mut self.tls.socket, READ_MESSAGE, Interest::READABLE)?;

        let mut events = Events::with_capacity(128);
        info!("Starting IO writer loop");
        let mut shutdown = false;
        while !shutdown {
            //Check for events
            poll.poll(&mut events, Some(Duration::from_millis(100)))
                .expect("Failed to poll for new events");
            for event in events.iter() {
                info!("Received event for {:?}", event);
                if let READ_MESSAGE = event.token() {
                    if event.is_readable() {
                        match self.read_data() {
                            Ok(messages) => {
                                for msg in messages {
                                    if let Err(err) = incoming_messages.send(msg) {
                                        error!("Send message to user thread failed {}", err);
                                        shutdown = true;
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to read data {}", e);
                                shutdown = true;
                            }
                        }
                    } else {
                        warn!("Non readable event {:?}", event);
                    }
                }
            }
            //Check if tls buffer needs writing
            //TODO Move check write function out of crypto
            if let Err(e) = self.tls.check_write() {
                error!("Failed to write tls ({})", e);
                shutdown = true;
            }

            //Send messages
            let mut messages: Vec<Message> = outgoing_message.try_iter().collect();
            messages.retain(|msg| {
                if msg.options == MessageOptions::Shutdown {
                    shutdown = true;
                    false
                } else {
                    //TODO Move write function out of crypto
                    match self.tls.write_message(msg) {
                        Ok(_) => false,
                        Err(e) => {
                            error!("Failed to send message ({}), ({})", msg, e);
                            true
                        }
                    }
                }
            });

            for msg in messages {
                warn!("Failed to send {}", msg);
            }
        }
        info!("Starting shutdown of IO thread");
        trace!("Checking for unparsed data in buffers");
        if !self.client_buffer.is_empty() {
            info!("Client buffer for {} is not empty!!!", self.addr);
            trace!("Dumping buffer\n{:?}", self.client_buffer);
            match self.read_data() {
                Ok(message) => {
                    for msg in message {
                        println!("Got {}", msg);
                        if let Err(e) = incoming_messages.send(msg) {
                            error!("Send message to user thread failed {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed getting data from buffer {}", e);
                }
            }
            if let Err(e) = self.tls.shutdown() {
                error!("Failed to close {}, as ({})", self.addr, e);
            };

            info!("Closed IO thread");
        }
        Ok(())
    }
    ///Reads group of messages from a client
    fn read_data(&mut self) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
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
    fn get_messages_from_buffer(&mut self) -> Vec<Message> {
        trace!("Getting messages from buffer");
        let mut messages = Vec::new();
        while self.client_buffer.len() > 2 {
            let cloned_buffer = self.client_buffer.clone();
            let (size, buffer) = cloned_buffer.split_at(2);
            let data_size = u16::from_be_bytes([size[0], size[1]]);
            trace!(
                "Message size is {}, buffer size {}",
                data_size,
                buffer.len()
            );
            if buffer.len() >= data_size as usize {
                let (msg_bytes, remaining_bytes) = buffer.split_at(data_size as usize).clone();

                let msg = Message::new(
                    String::from_utf8(msg_bytes.to_vec()).expect("Invalid utf-8 received"),
                    self.uuid,
                    false,
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
