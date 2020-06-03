use crate::client_handler::{Client, ClientIo};
use crate::crypto::TlsServer;
use crate::{Message, MessageOptions, MAX_CLIENTS_THREAD};

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

use log::{error, info, trace, warn};
use mio::{Events, Interest, Poll, Token};
use uuid::Uuid;

/// Struct representing a mio poll for incoming connections
///AND for distributing messages to the correct ClientIO thread
pub struct NetworkWorker {
    current_connection_count: u32,
    thread_count: u32,
    client_map: HashMap<Uuid, Sender<Message>>,
    client_io_shutdown_senders: Vec<Sender<Message>>,
    full_client_sender: Vec<Sender<Client>>,
    slave_client_sender: Sender<Client>,
    slave_messages_out_sender: Sender<Message>,
    master_messages_in: Sender<Message>,
    master_client_address: Sender<Uuid>,
}

impl NetworkWorker {
    pub fn init(
        master_client_in_sender: Sender<Uuid>,
        master_messages_in_sender: Sender<Message>,
    ) -> NetworkWorker {
        //Start io thread
        let (io_client_sender, io_client_receiver) = channel();
        let (io_messages_out_sender, io_messages_out_receiver) = channel();
        let mut shutdown_senders = Vec::new();
        shutdown_senders.push(io_messages_out_sender.clone());
        let slave_messages_in = master_messages_in_sender.clone();
        thread::Builder::new()
            .name(String::from("Client IO -01"))
            .spawn(move || {
                trace!("Created IO thread");
                let mut client_io = ClientIo::new(
                    io_client_receiver,
                    slave_messages_in,
                    io_messages_out_receiver,
                );
                client_io.start().expect("Client IO failed");
            })
            .expect("Failed to start client IO thread");
        NetworkWorker {
            current_connection_count: 0,
            thread_count: 0,
            client_map: HashMap::new(),
            full_client_sender: Vec::new(),
            slave_client_sender: io_client_sender,
            slave_messages_out_sender: io_messages_out_sender,
            master_messages_in: master_messages_in_sender,
            master_client_address: master_client_in_sender,
            client_io_shutdown_senders: shutdown_senders,
        }
    }

    ///Mio poll for incoming connections and distributing outgoing messages to correct client_io thread
    pub fn start(&mut self, address: String, messages_out: Receiver<Message>) {
        info!("Starting listening server on {}", address);
        let mut poll = Poll::new().unwrap();
        let mut events = Events::with_capacity(128);
        let mut tcp_listener = mio::net::TcpListener::bind(address.parse().unwrap()).unwrap();
        poll.registry()
            .register(&mut tcp_listener, Token(1), Interest::READABLE)
            .expect("Failed to network listener event");
        let mut tls_server = TlsServer::new(tcp_listener);

        loop {
            poll.poll(&mut events, Some(Duration::from_millis(100)))
                .expect("Network listener polling failed");
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
            for msg in messages_out.try_iter() {
                if msg.options == MessageOptions::Shutdown {
                    for shutdown in &mut self.client_io_shutdown_senders {
                        shutdown
                            .send(Message::shutdown())
                            .expect("Failed to send shutdown message");
                    }
                    info!("Closed listening thread");
                    return;
                } else {
                    self.client_map
                        .get_mut(&msg.client)
                        .unwrap()
                        .send(msg)
                        .expect("Failed to send message");
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
            self.full_client_sender
                .push(self.slave_client_sender.clone());
            self.slave_client_sender = new_client_sender;

            let (new_messages_out_sender, messages_out_receiver) = channel();
            self.client_io_shutdown_senders
                .push(new_messages_out_sender.clone());
            self.slave_messages_out_sender = new_messages_out_sender;
            let mut thread_name = String::from("Client IO - ");
            let slave_messages_in = self.master_messages_in.clone();
            thread_name.push_str(self.thread_count.to_string().as_str());
            thread::Builder::new()
                .name(thread_name)
                .spawn(|| {
                    trace!("Created IO thread");
                    let mut client_io =
                        ClientIo::new(client_receiver, slave_messages_in, messages_out_receiver);
                    client_io.start().expect("Client IO thread failed");
                })
                .expect("Failed to start new IO thread");
            self.thread_count += 1;
        }
        self.current_connection_count += 1;
        self.client_map
            .insert(new_client.uuid, self.slave_messages_out_sender.clone());
        info!("Sending {} to main thread", new_client.addr.to_string());
        let res = self.master_client_address.send(new_client.uuid); //.unwrap();//("Failed to send client address to user thread");
        if res.is_err() {
            error!("{}", res.err().unwrap());
        }
        self.slave_client_sender
            .send(new_client)
            .expect("Failed to send client to IO thread");
    }
}
