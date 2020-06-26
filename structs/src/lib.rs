#[macro_use]
extern crate diesel;
extern crate bigdecimal;
extern crate chrono;
extern crate uuid;
pub mod models;
pub mod protos;
pub mod schema;

use crate::models::{Account, Transaction};
use protos::message::{
    Request, Request_RequestType, Request_ResultType, Request_oneof_detailed_type,
};
use rand::{Rng, RngCore, SeedableRng};
use std::fmt;
use uuid::{Builder, Uuid, Variant, Version};

/**
    Client uuid - An identifier for the socket connection
    User uuid   - An identifier for a authenticated user

**/
impl Request {
    pub fn new_from_fields(
        data: Vec<String>,
        client: Uuid,
        from_client: bool,
        request_type: Request_RequestType,
        token_id: &String,
        detailed_type: Option<Request_oneof_detailed_type>,
    ) -> Result<Request, std::io::Error> {
        let message = Request {
            field_type: request_type,
            user_id: Default::default(),
            client_id: client.to_string(),
            data: protobuf::RepeatedField::from_vec(data),
            from_client,
            detailed_type,
            token_id: String::from(token_id),
            unknown_fields: protobuf::UnknownFields::new(),
            cached_size: Default::default(),
        };
        Ok(message)
    }
    pub fn success_result(
        data: Vec<String>,
        client: String,
        result_type: Request_ResultType,
    ) -> Request {
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
        message
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

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Account Number:    {}\n\
             Sort Code:         {}\n\
             Balance:           {}",
            self.account_number, self.sort_code, self.balance
        )
    }
}
impl Account {
    pub fn simple_account() -> Account {
        Account {
            account_number: rand::thread_rng().gen_range(10000000, 99999999),
            user_uuid: Default::default(),
            balance: bigdecimal::FromPrimitive::from_i64(100).unwrap(),
            sort_code: 123456,
            interest_rate: bigdecimal::FromPrimitive::from_f64(1.0).unwrap(),
            overdraft_limit: bigdecimal::FromPrimitive::from_i64(50).unwrap(),
            account_name: None,
            account_category: None,
            archived: false,
        }
    }
}
impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Source Account:    {}\n\
             Dest Account:      {}\n\
             Amount:            {}",
            self.source_account_number, self.dest_account_number, self.amount
        )
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
