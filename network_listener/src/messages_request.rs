//TODO THIS IS A STATIC FILE COPIED FROM BUILD FOR CODE COMPLETION
pub mod asd {
    use crate::fmt;
    use prost_derive::Message;
    use std::fmt::Formatter;

    #[derive(Clone, PartialEq, Message)]
    pub struct Request {
        #[prost(enumeration = "request::RequestType", tag = "1")]
        pub r#type: i32,
        #[prost(string, tag = "2")]
        pub user_id: std::string::String,
        #[prost(string, tag = "3")]
        pub client_id: std::string::String,
        #[prost(string, repeated, tag = "4")]
        pub data: ::std::vec::Vec<std::string::String>,
        #[prost(bool, tag = "5")]
        pub from_client: bool,
        #[prost(oneof = "request::DetailedType", tags = "10, 11, 12, 13")]
        pub detailed_type: ::std::option::Option<request::DetailedType>,
    }
    impl fmt::Display for Request {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
            write!(f, "Request from {}", self.client_id)
        }
    }
    impl Request {
        fn send(&mut self) {
            self.enconded_len();
        }
    }

    pub mod request {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
        #[repr(i32)]
        pub enum RequestType {
            Shutdown = 0,
            Authenticate = 1,
            Transactions = 2,
            Account = 3,
            Misc = 4,
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
        #[repr(i32)]
        pub enum AuthenticateType {
            Login = 0,
            NewUser = 1,
            ChangePassword = 2,
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
        #[repr(i32)]
        pub enum TransactionType {
            ListTransactions = 0,
            TransactionInfo = 1,
            NewTransaction = 2,
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
        #[repr(i32)]
        pub enum AccountType {
            ListAccounts = 0,
            AccountInfo = 1,
            NewAccount = 2,
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
        #[repr(i32)]
        pub enum MiscType {
            Unknown = 0,
            NewClient = 1,
        }

        #[derive(Clone, PartialEq, ::prost::Oneof)]
        pub enum DetailedType {
            #[prost(enumeration = "AuthenticateType", tag = "10")]
            Auth(i32),
            #[prost(enumeration = "TransactionType", tag = "11")]
            Transaction(i32),
            #[prost(enumeration = "AccountType", tag = "12")]
            Account(i32),
            #[prost(enumeration = "MiscType", tag = "13")]
            Misc(i32),
        }
    }
}
