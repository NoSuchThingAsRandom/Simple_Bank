extern crate network_listener;

use log::error;
use log::info;
use network_listener::{
    protos::message::Request_RequestType, Client, ADDRESS, DATA_ACCOUNTS_PORT, DATA_MISC_PORT,
    DATA_TRANSACTIONS_PORT, LOAD_BALANCER_PORT,
};

pub struct Instance {
    network_server: network_listener::Server,
    data_accounts: Client,
    data_transactions: Client,
    data_misc: Client,
}

impl Instance {
    pub fn new() -> Instance {
        let mut server_address = String::from(ADDRESS);
        server_address.push_str(LOAD_BALANCER_PORT);
        let network = network_listener::Server::init(server_address);
        info!("Created load balancer instance");
        Instance {
            network_server: network,
            data_accounts: Instance::start_client(DATA_ACCOUNTS_PORT),
            data_transactions: Instance::start_client(DATA_TRANSACTIONS_PORT),
            data_misc: Instance::start_client(DATA_MISC_PORT),
        }
    }
    fn start_client(port: &str) -> Client {
        let mut address = String::from(ADDRESS);
        address.push_str(port);
        network_listener::Client::start(address)
    }

    pub fn start(&mut self) {
        loop {
            //Get incoming client messages
            for msg in self.network_server.get_messages() {
                match msg.field_type {
                    Request_RequestType::SHUTDOWN => {}
                    Request_RequestType::AUTHENTICATE => {
                        if let Err(e) = self.data_accounts.send_message(msg) {
                            error!("Failed to make request to data accounts ({})", e);
                        };
                    }
                    Request_RequestType::TRANSACTIONS => {
                        if let Err(e) = self.data_transactions.send_message(msg) {
                            error!("Failed to make request to data transactions ({})", e);
                        };
                    }
                    Request_RequestType::ACCOUNT => {
                        if let Err(e) = self.data_accounts.send_message(msg) {
                            error!("Failed to make request to data accounts ({})", e);
                        };
                    }
                    Request_RequestType::MISC => {
                        if let Err(e) = self.data_misc.send_message(msg) {
                            error!("Failed to make request to data misc ({})", e);
                        };
                    }
                }
            }
            //Forward outgoing messages from servers
            for msg in self.data_accounts.get_messages() {
                if let Err(e) = self.network_server.send_message(msg) {
                    error!(
                        "Failed to forward request from data accounts to network server ({})",
                        e
                    );
                };
            }
            for msg in self.data_transactions.get_messages() {
                if let Err(e) = self.network_server.send_message(msg) {
                    error!(
                        "Failed to forward request from data transactions to network server ({})",
                        e
                    );
                };
            }
            for msg in self.data_misc.get_messages() {
                if let Err(e) = self.network_server.send_message(msg) {
                    error!(
                        "Failed to forward request from data misc to network server ({})",
                        e
                    );
                };
            }
        }
    }
}
