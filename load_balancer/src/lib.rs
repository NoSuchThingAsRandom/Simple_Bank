extern crate database_handler;
extern crate network_listener;

use std::error::Error;
use std::process::exit;

use log::error;
use log::info;
use log::trace;
use log::warn;

use database_handler::models::User;
use database_handler::DbConnection;
use network_listener::protos::message::{
    Request, Request_AccountType, Request_AuthenticateType, Request_MiscType, Request_ResultType,
    Request_TransactionType, Request_oneof_detailed_type,
};
use network_listener::{
    protos::message::Request_RequestType, Client, ADDRESS, DATA_ACCOUNTS_PORT, DATA_MISC_PORT,
    DATA_TRANSACTIONS_PORT, LOAD_BALANCER_PORT,
};

//TODO Prevent against timing attacks
//https://blog.ircmaxell.com/2014/11/its-all-about-time.html
pub struct Instance {
    network_server: network_listener::Server,
    database_connection: database_handler::DbConnection,
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
            database_connection: database_handler::DbConnection::new_connection(),
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
                if match self.token_is_valid(&msg) {
                    Ok(auth) => auth,
                    Err(e) => {
                        warn!("Failed to verify auth {}", e);
                        false
                    }
                } {
                    //Parse user request
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
                            match msg.detailed_type {
                                Some(Request_oneof_detailed_type::account(
                                    Request_AccountType::LIST_ACCOUNTS,
                                )) => {}
                                Some(Request_oneof_detailed_type::account(
                                    Request_AccountType::ACCOUNT_INFO,
                                )) => if let Err(e) = self.get_account_info(&msg) {},

                                Some(Request_oneof_detailed_type::account(
                                    Request_AccountType::NEW_ACCOUNT,
                                )) => {}

                                Some(_) | None => {}
                            }
                            if let Err(e) = self.data_accounts.send_message(msg) {
                                error!("Failed to make request to data accounts ({})", e);
                            };
                        }
                        Request_RequestType::MISC => {
                            if let Err(e) = self.data_misc.send_message(msg) {
                                error!("Failed to make request to data misc ({})", e);
                            };
                        }
                        Request_RequestType::Result => {}
                    }
                } else {
                    if msg.field_type == Request_RequestType::AUTHENTICATE {
                        match msg.detailed_type {
                            Some(Request_oneof_detailed_type::auth(
                                Request_AuthenticateType::LOGIN,
                            )) => {
                                if let Err(e) = self.login(&msg) {
                                    warn!(
                                        "Failed to login client ({}) with error ({})",
                                        msg.client_id, e
                                    )
                                };
                            }
                            Some(Request_oneof_detailed_type::auth(
                                Request_AuthenticateType::NEW_USER,
                            )) => {
                                if let Err(e) = self.create_user(&msg) {
                                    warn!(
                                        "Failed to login client ({}) with error ({})",
                                        msg.client_id, e
                                    );
                                };
                            }
                            Some(_) | None => {
                                self.send_login_request(&msg);
                            }
                        }
                    } else {
                        self.send_login_request(&msg);
                    }
                }
            }
        }
    }
    fn send_critical_error(&mut self, client_id: String) -> Result<(), Box<dyn Error>> {
        let request =
            Request::success_result(Vec::new(), client_id, Request_ResultType::UNEXPECTED_ERROR)?;
        self.network_server.send_message(request)?;
        Ok(())
    }
    fn send_incorrect_arguments_error(&mut self, client_id: String) -> Result<(), Box<dyn Error>> {
        let request = match Request::success_result(
            Vec::new(),
            client_id,
            Request_ResultType::INVALID_ARGS,
        ) {
            Ok(req) => req,
            Err(e) => {
                self.send_critical_error(client_id.clone());
                return Err(Box::new(e));
            }
        };
        if let Err(e) = self.network_server.send_message(request) {
            self.send_critical_error(client_id.clone());
            return Err(Box::new(e));
        };
        Ok(())
    }

    fn send_login_request(&mut self, msg: &Request) {
        //Send login request
        trace!("Message not authenticated, sending login request");
        let auth_req = Request {
            field_type: Request_RequestType::AUTHENTICATE,
            user_id: msg.user_id.clone(),
            client_id: msg.client_id.clone(),
            data: Default::default(),
            from_client: false,
            token_id: "".to_string(),
            detailed_type: Some(Request_oneof_detailed_type::auth(
                Request_AuthenticateType::LOGIN,
            )),
            unknown_fields: Default::default(),
            cached_size: Default::default(),
        };
        if let Err(e) = self.network_server.send_message(auth_req) {
            error!("Failed to send login message to {}", &msg.client_id);
            self.send_critical_error(msg.client_id.clone()).unwrap();
        }
    }

    fn create_user(&mut self, msg: &Request) -> Result<(), Box<dyn Error>> {
        let mut user: User = serde_json::from_str(msg.data.get(0).unwrap())?;
        user.user_uuid = database_handler::new_secure_uuid_v4();
        user.join_date = chrono::Utc::today().naive_utc();
        user.archived = false;
        let a = if self.database_connection.new_user_account(&user).is_err() {
            warn!("Failed to create new user");
            if self
                .send_incorrect_arguments_error(msg.client_id.clone())
                .is_err()
            {
                self.send_critical_error();
                Ok(())
            } else {
                Ok(())
            }
        } else {
            Ok(())
        };
        Ok(())
    }

    fn login(&mut self, msg: &Request) -> Result<(), Box<dyn Error>> {
        let token_req = self.database_connection.login(
            String::from(msg.data.get(0).ok_or("")?),
            String::from(msg.data.get(0).ok_or("")?),
        );

        let result: Request = match token_req {
            Ok(token_opt) => match token_opt {
                Some(token) => {
                    let mut data = Vec::new();
                    data.push(token);
                    Request::success_result(
                        data,
                        msg.client_id.clone(),
                        Request_ResultType::SUCCESS,
                    )?
                }
                None => Request::success_result(
                    Vec::new(),
                    msg.client_id.clone(),
                    Request_ResultType::INVALID_ARGS,
                )?,
            },

            Err(E) => Request::success_result(
                Vec::new(),
                msg.client_id.clone(),
                Request_ResultType::UNEXPECTED_ERROR,
            )?,
        };
        self.network_server.send_message(result)?;
        Ok(())
    }

    /** Validates a token
            Ok(True) - The token is valid
            Ok(False) -  The token has expired/invalid
            Err - The token doesn't exist/unexpected error
    **/
    fn token_is_valid(&mut self, msg: &Request) -> Result<bool, Box<dyn std::error::Error>> {
        return Ok(self
            .database_connection
            .check_token(msg.token_id.parse()?, msg.user_id.parse()?)?);
    }

    fn get_account_info(&mut self, msg: &Request) -> Result<(), Box<dyn Error>> {
        let account = self
            .database_connection
            .get_bank_account(msg.data[0].parse()?, msg.user_id.parse()?)?;
        let mut data: Vec<String> = Vec::new();
        data.push(serde_json::to_string(&account)?);
        Request::success_result(data, msg.client_id.clone(), Request_ResultType::SUCCESS)?;
        Ok(())
    }
}
