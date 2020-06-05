use log::{error, info, trace, warn};

use network_listener::{
    protos::message::Request, Client, ADDRESS, DATA_ACCOUNTS_PORT, DATA_MISC_PORT,
    DATA_TRANSACTIONS_PORT, LOAD_BALANCER_PORT,
};
use std::sync::atomic::Ordering::AcqRel;
use std::sync::mpsc::{channel, Receiver, Sender};

pub enum DataTypes {
    Shutdown,
    NewLoadBalancer,
    NewClientIO,
    NewClient,
}

pub struct RequestOLD {
    data_type: DataTypes,
    pub(crate) data: String,
    options: String,
}

pub struct Communications {
    data_in: Receiver<Request>,
    data_out: Sender<Request>,
    name: String,
}

pub struct Instance {
    client_listener: network_listener::Server,
    data_accounts: Client,
    data_transactions: Client,
    data_misc: Client,
}

impl Instance {
    pub fn new() -> Instance {
        let mut server_address = String::from(ADDRESS);
        server_address.push_str(LOAD_BALANCER_PORT);
        let network = network_listener::Server::init(server_address);

        Instance {
            client_listener: network,
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
            for msg in self.client_listener.get_messages() {
                let sep_data: Vec<&str> = msg.data.get(0).unwrap().split("://").collect();
                if sep_data.len() < 2 {
                    warn!("Invalid data received!");
                    continue;
                }
                match sep_data.get(0).unwrap() {
                    &"LOGIN" => trace!("User ({}) requested login", msg.client_id),
                    &"LIST_ACCOUNTS" => {
                        trace!("User ({}) requested list of account ", msg.client_id)
                    }
                    &"ACCOUNT" => trace!(
                        "User ({}) requested detailed info of account ({}) ",
                        msg.client_id,
                        sep_data.get(1).unwrap()
                    ),
                    &_ => {}
                }
            }
        }
    }
}

pub struct Data {
    balancers: Vec<Communications>,
}

impl Data {
    pub fn new() -> Data {
        Data {
            balancers: Vec::new(),
        }
    }
    pub fn start(&mut self) {
        //let (sender, receiver) = channel();
        //let (sender2, receiver2) = channel();
        //sender.send(receiver2);
        //sender.send(String::from("Hello"));
    }
}
