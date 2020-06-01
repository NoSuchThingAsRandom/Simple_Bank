use std::collections::HashMap;
use std::sync::mpsc::*;
use std::thread;
use std::time::Duration;

use log::{error, info, trace, warn};
use mio::{Interest, Poll, Token};
use mio::Events;

use crate::{Message, MessageOptions};
use crate::crypto::{TlsConnection, TlsServer};

//pub(crate) const ADDR: &str = "127.0.0.1:5962";
const MAX_CLIENTS_THREAD: u8 = 20;
//const SERVER: Token = Token(11);
pub(crate) const MAX_MESSAGE_BYTES: u16 = 65535;
const BUFFER_SIZE: usize = 512;

/// Struct representing a singular client
pub struct Client {
    pub addr: String,
    client_buffer: Vec<u8>,
    tls: TlsConnection,
}

impl Client {
    pub fn new(addr: String, tls: TlsConnection) -> Client {
        Client { addr, client_buffer: Vec::new(), tls }
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
        }
    }


    ///Attempts to export messages from internal buffer
    pub fn get_messages_from_buffer(&mut self) -> Vec<Message> {
        trace!("Getting messages from buffer");
        let mut messages = Vec::new();
        while self.client_buffer.len() > 2 {
            let cloned_buffer = self.client_buffer.clone();
            let (size, buffer) = cloned_buffer.split_at(2);
            let data_size = u16::from_be_bytes([size[0], size[1]]);
            trace!("Message size is {}, buffer size {}", data_size, buffer.len());
            if buffer.len() >= data_size as usize {
                let (msg_bytes, remaining_bytes) = buffer.split_at(data_size as usize).clone();

                let msg = Message::new(String::from_utf8(msg_bytes.to_vec()).expect("Invalid utf-8 received"), self.addr.to_string(), self.addr.clone()).unwrap();
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
struct ClientIo {
    clients: HashMap<Token, Client>,
    current_client_count: usize,
    poll: Poll,
    incoming_clients: Receiver<Client>,
    messages_in: Sender<Message>,
    messages_out: Receiver<Message>,
}

impl ClientIo {
    fn new(incoming_clients: Receiver<Client>, messages_in: Sender<Message>, messages_out: Receiver<Message>) -> ClientIo {
        ClientIo { clients: HashMap::new(), current_client_count: 0, poll: Poll::new().unwrap(), incoming_clients, messages_in, messages_out }
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
            let mut messages: Vec<Message> = self.messages_out.try_iter().collect();
            for client in self.clients.values_mut() {
                messages.retain(|msg|
                    if msg.options == MessageOptions::Shutdown {
                        shutdown = true;
                        false
                    } else {
                        if client.addr == msg.recipient {
                            match client.tls.write_message(msg) {
                                Ok(_) => { false }
                                Err(e) => {
                                    error!("Failed to send message ({}), ({})", msg, e);
                                    true
                                }
                            }
                        } else {
                            true
                        }
                    });
            }
            for msg in messages {
                warn!("Failed to send {}", msg);
            }

            //Check for incoming messages
            self.poll.poll(&mut events, Some(Duration::from_millis(100))).expect("Failed to poll for new events");
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
                    error!("Failed getting data from buffer {}", messages.err().unwrap());
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
                    self.poll.registry().register(&mut c.tls.socket, token, Interest::READABLE).unwrap();
                    self.clients.insert(token, c);
                }
                Err(e) => if e == TryRecvError::Empty {
                    new_clients = false;
                } else {
                    return Err(e);
                },
            }
        }
        Ok(())
    }


}

/// Struct representing a mio poll for incoming connections
///AND for distributing messages to the correct ClientIO thread
struct NetworkWorker {
    current_connection_count: u32,
    thread_count: u32,
    client_map: HashMap<String, Sender<Message>>,
    client_io_shutdown_senders: Vec<Sender<Message>>,
    full_client_sender: Vec<Sender<Client>>,
    slave_client_sender: Sender<Client>,
    slave_messages_out_sender: Sender<Message>,
    master_messages_in: Sender<Message>,
    master_client_address: Sender<String>,

}

impl NetworkWorker {
    fn init(master_client_address: Sender<String>, messages_in: Sender<Message>) -> NetworkWorker {
        master_client_address.send(String::from("HEy")).unwrap();
        //Start io thread
        let (client_sender, client_receiver) = channel();
        let (messages_out_sender, messages_out_receiver) = channel();
        let mut shutdown_senders = Vec::new();
        shutdown_senders.push(messages_out_sender.clone());
        let slave_messages_in = messages_in.clone();
        thread::Builder::new().name(String::from("Client IO")).spawn(move || {
            trace!("Created IO thread");
            let mut client_io = ClientIo::new(client_receiver, slave_messages_in, messages_out_receiver);
            client_io.start().expect("Client IO failed");
        }).expect("Failed to start initial client IO thread");
        NetworkWorker { current_connection_count: 0, thread_count: 0, client_map: HashMap::new(), full_client_sender: Vec::new(), slave_client_sender: client_sender, slave_messages_out_sender: messages_out_sender, master_messages_in: messages_in, master_client_address, client_io_shutdown_senders: shutdown_senders }
    }

    ///Mio poll for incoming connections and distributing outgoing messages to correct client_io thread
    fn start(&mut self, address: String, clients_in: Receiver<Client>, messages_out: Receiver<Message>) {
        info!("Starting listening server on {}", address);
        let mut poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(128);
        let mut tcp_listener = mio::net::TcpListener::bind(address.parse().unwrap()).unwrap();
        poll.registry().register(&mut tcp_listener, Token(1), Interest::READABLE).expect("Failed to network listener event");
        let mut tls_server = TlsServer::new(tcp_listener);

        loop {
            poll.poll(&mut events, Some(Duration::from_millis(100))).expect("Network listener polling failed");
            for event in events.iter() {
                match event.token() {
                    Token(1) => {
                        let client = tls_server.accept_connection();
                        self.add_client(client);
                    }
                    Token(_) => {
                        error!("Unknown event requested");
                        unimplemented!("Unknown event requested!");
                    }
                }
            }
            for client in clients_in.try_iter() {
                self.add_client(client);
            }
            for msg in messages_out.try_iter() {
                if msg.options == MessageOptions::Shutdown {
                    for shutdown in &mut self.client_io_shutdown_senders {
                        shutdown.send(Message::shutdown()).expect("Failed to send shutdown message");
                    }
                    info!("Closed listening thread");
                    return;
                } else {
                    self.client_map.get_mut(msg.recipient.as_str()).unwrap().send(msg).expect("Failed to send message");
                }
            }
        }
    }

    /// Sends a new incoming client to a client_io thread
    /// Creates a new client_io thread, if all client_io instances are at max capacity
    fn add_client(&mut self, new_client: Client) {
        if self.current_connection_count >= MAX_CLIENTS_THREAD as u32 {
            //Update handlers
            let (new_client_sender, client_receiver) = channel();
            self.full_client_sender.push(self.slave_client_sender.clone());
            self.slave_client_sender = new_client_sender;

            let (new_messages_out_sender, messages_out_receiver) = channel();
            self.client_io_shutdown_senders.push(new_messages_out_sender.clone());
            self.slave_messages_out_sender = new_messages_out_sender;
            let mut thread_name = String::from("Client IO - ");
            let slave_messages_in = self.master_messages_in.clone();
            thread_name.push_str(self.thread_count.to_string().as_str());
            thread::Builder::new().name(thread_name).spawn(|| {
                trace!("Created IO thread");
                let mut client_io = ClientIo::new(client_receiver, slave_messages_in, messages_out_receiver);
                client_io.start().expect("Client IO thread failed");
            }).expect("Failed to start new IO thread");
            self.thread_count += 1;
        }
        self.current_connection_count += 1;
        self.client_map.insert(new_client.addr.to_string(), self.slave_messages_out_sender.clone());
        info!("Sending {} to main thread", new_client.addr.to_string());
        let res = self.master_client_address.send(new_client.addr.to_string());//.unwrap();//("Failed to send client address to user thread");
        if res.is_err() {
            error!("{}", res.err().unwrap());
        }
        self.slave_client_sender.send(new_client).expect("Failed to send client to IO thread");
    }
}

/// Initiation struct for network worker and holding all publicly accessible variables
pub struct Network {
    pub client_sender: Sender<Client>,
    pub connections: Vec<Client>,

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
    pub fn init(address: String, client_addr: Sender<String>, messages_in: Sender<Message>, master_messages_out: Receiver<Message>) -> Network {
        info!("Creating network handler");
        let (master_client_sender, master_client_receiver) = channel();
        thread::Builder::new().name(String::from("Listening Server")).spawn(move || {
            let mut worker = NetworkWorker::init(client_addr, messages_in);
            worker.start(address, master_client_receiver, master_messages_out);
        }).expect("Failed to start network listener thread");

        //Start listening server


//Return network instance (With client sender for initiated connections)
        Network { client_sender: master_client_sender, connections: Vec::new() }
    }
}