use std::sync::mpsc::{channel, Receiver, Sender};

pub enum DataTypes {
    Shutdown,
    NewLoadBalancer,
    NewClientIO,
    NewClient,
}

pub struct Request {
    data_type: DataTypes,
    options: String,
}

pub struct Communications {
    data_in: Receiver<Request>,
    data_out: Sender<Request>,
    name: String,
}
pub struct NewInstance {
    request_in: Receiver<Sender<Communications>>,
    request_out: Sender<Receiver<Communications>>,
    name: String,
}
impl NewInstance {
    pub fn new() -> NewInstance {
        let (request_out, request_in) = channel();
        NewInstance {
            request_in,
            request_out,
            name: "".to_string(),
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
        let (sender, receiver) = channel();
        let (sender2, receiver2) = channel();
        sender.send(receiver2);
        sender.send(String::from("Hello"));
    }
}
