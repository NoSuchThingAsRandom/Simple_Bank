extern crate chrono;
extern crate network_listener;
extern crate protobuf;
extern crate serde_json;
extern crate structs;
extern crate strum;
extern crate strum_macros;

use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;
use std::sync::mpsc::{channel, Receiver, Sender};

use log::{error, info, trace, warn};
use protobuf::RepeatedField;
use structs::models::{Account, User};
use structs::protos::message::{
    Request, Request_AccountType, Request_AuthenticateType, Request_RequestType,
    Request_ResultType, Request_oneof_detailed_type,
};
use strum::EnumMessage;
use strum::IntoEnumIterator;
use strum_macros::AsRefStr;
use strum_macros::EnumIter;
use strum_macros::EnumMessage;
use strum_macros::EnumString;
use text_io::read;

use network_listener::Client;

#[derive(EnumIter, EnumString, EnumMessage, Debug, AsRefStr)]
enum Commands {
    #[strum(
        message = "List_Accounts",
        detailed_message = "This list all your bank accounts"
    )]
    List_Accounts,
    #[strum(
        message = "Create_Account",
        detailed_message = "This will create a account"
    )]
    Create_Account,

    #[strum(
        message = "Account_Info",
        detailed_message = "This provides detailed information about an account"
    )]
    Account_Info,

    #[strum(message = "Exit", detailed_message = "This exits the program")]
    Exit,
}

impl Commands {
    fn get_help_dialog() -> String {
        let mut out = String::new();
        out.push_str("Possible commands:\n");
        for command in Commands::iter() {
            out.push_str(
                format!(
                    "      {} - {}\n",
                    command.get_message().unwrap_or(command.as_ref()),
                    command
                        .get_detailed_message()
                        .unwrap_or("No Help message :(")
                )
                .as_ref(),
            );
        }
        return out;
    }
    fn get_user_command() -> Commands {
        println!("\n\nEnter command: ");
        let input: String = read!("{}\n");
        match Commands::from_str(&input) {
            Ok(t) => t,
            Err(_) => {
                println!("Invalid option!\n\n{}", Commands::get_help_dialog());
                Commands::get_user_command()
            }
        }
    }
}

pub struct InputLoop {
    server_connection: network_listener::Client,
    token: String,
    archive_message: Vec<Request>,
}

impl InputLoop {
    pub fn new(address: String) -> InputLoop {
        InputLoop {
            token: String::from(""),
            server_connection: Client::start(address),
            archive_message: Vec::new(),
        }
    }
    ///The main user input loop
    /// Responds to user input
    pub fn start(&mut self) {
        info!("Started user input loop");
        println!("Would you like to login (login) or create an account (create)? ");
        let choice: String = read!("{}\n");
        if choice == String::from("login") {
            self.login();
        } else {
            self.new_user_account();
        }
        loop {
            match Commands::get_user_command() {
                Commands::List_Accounts => {
                    self.list_accounts();
                }
                Commands::Account_Info => {
                    self.account_info();
                }
                Commands::Exit => {
                    println!("Goodbye!");
                    return;
                }
                Commands::Create_Account => {
                    self.new_bank_account();
                }
            }
        }
    }
    fn process_message(&mut self, msg: &Request) {
        match &msg.field_type {
            Request_RequestType::AUTHENTICATE => {
                println!("You are not logged in");
                self.login()
            }
            Request_RequestType::ACCOUNT => {
                if let Some(detail) = &msg.detailed_type {
                    match detail {
                        Request_oneof_detailed_type::auth(_) => {}
                        Request_oneof_detailed_type::transaction(_) => {}
                        Request_oneof_detailed_type::account(_) => {}
                        Request_oneof_detailed_type::misc(_) => {}
                        Request_oneof_detailed_type::result(_) => {}
                    }
                }
            }
            Request_RequestType::SHUTDOWN => {}
            Request_RequestType::TRANSACTIONS => {}
            Request_RequestType::MISC => {}
            Request_RequestType::Result => {}
        }
    }
    fn login(&mut self) {
        println!("Enter your username: ");
        let username: String = read!("{}\n");
        println!("Enter your password: ");
        let password: String = read!("{}\n");
        let data = vec![username, password];
        let msg = Request {
            field_type: Request_RequestType::AUTHENTICATE,
            user_id: "".to_string(),
            client_id: "".to_string(),
            data: RepeatedField::from_vec(data),
            from_client: true,
            token_id: "".to_string(),
            detailed_type: Some(Request_oneof_detailed_type::auth(
                Request_AuthenticateType::LOGIN,
            )),
            unknown_fields: Default::default(),
            cached_size: Default::default(),
        };
        self.server_connection.send_message(msg).unwrap();
        if let Some(data) = InputLoop::process_result(self.server_connection.get_singular_message())
        {
            self.token = String::from(data.get(0).unwrap());
            println!("Successfully logged in");
        } else {
            println!("Failed to login!");
            self.login();
        }
    }

    fn new_user_account(&mut self) {
        println!("Enter your username: ");
        let username: String = read!("{}\n");

        println!("Enter your password: ");
        let password: String = read!("{}\n");

        println!("Enter your email: ");
        let email: String = read!("{}\n");

        println!("Enter your date of birth in the format 'DD MM YYYY: ");
        let dob_str: String = read!("{}\n");
        let dob = match chrono::NaiveDate::parse_from_str(&dob_str, "%d %m %Y") {
            Ok(dob) => dob,
            Err(E) => {
                println!("Invalid date format\nExiting user creation");
                return;
            }
        };
        let user: User = User {
            user_uuid: Default::default(),
            username,
            password,
            email,
            date_of_birth: dob,
            join_date: chrono::Utc::today().naive_utc(),
            archived: false,
        };
        let user_string = match serde_json::to_string(&user) {
            Ok(user_str) => user_str,
            Err(E) => {
                println!("Failed to process given data\nExiting user creation");
                return;
            }
        };
        let msg = Request {
            field_type: Request_RequestType::AUTHENTICATE,
            user_id: "".to_string(),
            client_id: "".to_string(),
            data: RepeatedField::from_vec(vec![user_string]),
            from_client: true,
            token_id: "".to_string(),
            detailed_type: Some(Request_oneof_detailed_type::auth(
                Request_AuthenticateType::NEW_USER,
            )),
            unknown_fields: Default::default(),
            cached_size: Default::default(),
        };
        self.server_connection.send_message(msg).unwrap();
        let response = self.server_connection.get_singular_message();
        if let Some(_) = InputLoop::process_result(response) {
            println!("Successfully created account");
            println!("Now logging in...");
            self.login();
        } else {
            println!("Failed to create account");
        }
    }
    fn new_bank_account(&mut self) {
        let data = Vec::new();
        let request = Request::new_from_fields(
            data,
            Default::default(),
            true,
            Request_RequestType::ACCOUNT,
            &self.token,
            Some(Request_oneof_detailed_type::account(
                Request_AccountType::NEW_ACCOUNT,
            )),
        )
        .unwrap();
        self.server_connection.send_message(request).unwrap();
        let response = self.server_connection.get_singular_message();
        if let Some(_) = InputLoop::process_result(response) {
            println!("Successfully created bank account");
        } else {
            println!("Failed to create bank account");
        }
    }

    fn account_info(&mut self) {}
    fn list_accounts(&mut self) {
        let data = Vec::new();
        let request = Request::new_from_fields(
            data,
            Default::default(),
            true,
            Request_RequestType::ACCOUNT,
            &self.token,
            Some(Request_oneof_detailed_type::account(
                Request_AccountType::LIST_ACCOUNTS,
            )),
        )
        .unwrap();
        self.server_connection.send_message(request).unwrap();
        if let Some(data) = InputLoop::process_result(self.server_connection.get_singular_message())
        {
            println!("{:?}", &data);
            println!("Accounts: ");
            println!("Account Number        Balance     ");
            let accounts: Vec<Account> = serde_json::from_str(data.get(0).unwrap()).unwrap();
            for account_str in accounts {
                println!("{}", account_str);
            }
        }
    }

    fn process_result(request: Request) -> Option<Vec<String>> {
        if let Some(Request_oneof_detailed_type::result(result_type)) = request.detailed_type {
            match result_type {
                Request_ResultType::SUCCESS => return Some(Vec::try_from(request.data).unwrap()),
                Request_ResultType::INVALID_ARGS => {
                    println!("Invalid details given");
                }
                Request_ResultType::UNEXPECTED_ERROR => {
                    println!("Unexpected error encountered!");
                }
                Request_ResultType::NOT_AUTHENTICATED => {
                    println!("Not authenticated?\nSHIT BROKE");
                }
            }
        } else {
            println!("Invalid return type given!!!\n SHIT VERY BROKE");
        }
        None
    }

    pub fn shutdown(&mut self) {
        //self.messages_out_sender
        //    .send(Message::shutdown())
        //    .expect("Failed to send shutdown command");
    }
}
