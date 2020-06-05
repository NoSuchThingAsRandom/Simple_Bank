use protobuf::Message;

mod message;
fn main() {
    let mut req = message::Request::new();
    req.set_client_id(String::from("Hello"));
    req.write_length_delimited_to_vec();
    println!("HEllo world\n{:?}", req);
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
