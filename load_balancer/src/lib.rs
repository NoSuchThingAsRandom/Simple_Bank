extern crate database_handler;
extern crate network_listener;
extern crate structs;

use std::fmt;
use std::fmt::Formatter;

use log::error;
use log::info;
use log::trace;
use log::warn;
use structs::models::{Account, User};
use structs::protos::message::{
    Request, Request_AccountType, Request_AuthenticateType, Request_MiscType, Request_RequestType,
    Request_ResultType, Request_TransactionType, Request_oneof_detailed_type,
};

use database_handler::{DatabaseError, DatabaseErrorKind};
use network_listener::{
    Client, ADDRESS, DATA_ACCOUNTS_PORT, DATA_MISC_PORT, DATA_TRANSACTIONS_PORT, LOAD_BALANCER_PORT,
};

struct RequestError {
    error_kind: ErrorKind,
}

impl RequestError {
    fn new(error_kind: ErrorKind) -> RequestError {
        RequestError { error_kind }
    }

    fn process(&mut self, instance: &mut Instance, client_id: String) -> bool {
        trace!("Processing error ({:?})", self);
        match self.error_kind {
            ErrorKind::NotAuthenticated => {
                if instance.send_authentication_error(client_id).is_err() {
                    error!("The network connection has failed");
                    return true;
                }
                return false;
            }

            ErrorKind::DatabaseFailure => {
                instance.send_critical_error(client_id);
                panic!("The database has crashed!");
            }
            ErrorKind::Shutdown => {
                instance.send_critical_error(client_id);
                instance.database_connection.close();
                return true;
            }
            ErrorKind::InvalidArguments => {
                if instance.send_incorrect_arguments_error(client_id).is_err() {
                    error!("The network connection has failed");
                    return true;
                }
                return false;
            }
            ErrorKind::NetworkFailure => {
                instance.send_critical_error(client_id);
                error!("The network connection has failed!");
                return true;
            }
        };
        error!("The error was not processed!");
        false
    }
}

impl From<database_handler::DatabaseError> for RequestError {
    fn from(error: DatabaseError) -> Self {
        match error.error_kind {
            database_handler::DatabaseErrorKind::AlreadyExists => {
                RequestError::new(ErrorKind::InvalidArguments)
            }

            DatabaseErrorKind::AuthenticationError => {
                RequestError::new(ErrorKind::NotAuthenticated)
            }
            DatabaseErrorKind::DatabaseFailure => RequestError::new(ErrorKind::DatabaseFailure),
            DatabaseErrorKind::Unknown => RequestError::new(ErrorKind::DatabaseFailure),
            DatabaseErrorKind::NotFound => RequestError::new(ErrorKind::InvalidArguments),
        }
    }
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.error_kind {
            ErrorKind::NotAuthenticated => write!(f, "The request was not authenticated properly!"),
            ErrorKind::InvalidArguments => write!(f, "Invalid arguments were provided"),
            ErrorKind::DatabaseFailure => write!(
                f,
                "An unexpected error occurred with the database connection"
            ),
            ErrorKind::NetworkFailure => write!(
                f,
                "An unexpected error occurred with the network connection"
            ),
            ErrorKind::Shutdown => write!(f, "Need to shutdown"),
        }
    }
}

impl fmt::Debug for RequestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for RequestError {}

enum ErrorKind {
    NotAuthenticated,
    InvalidArguments,
    DatabaseFailure,
    NetworkFailure,
    Shutdown,
}

//TODO Prevent against timing attacks
//https://blog.ircmaxell.com/2014/11/its-all-about-time.html
pub struct Instance {
    network_server: network_listener::Server,
    database_connection: database_handler::DbConnection,
    /*    data_accounts: Client,
    data_transactions: Client,
    data_misc: Client,*/
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
            /*            data_accounts: Instance::start_client(DATA_ACCOUNTS_PORT),
            data_transactions: Instance::start_client(DATA_TRANSACTIONS_PORT),
            data_misc: Instance::start_client(DATA_MISC_PORT),*/
        }
    }
    /// Function that pulls request from the mpsc queue, and validates the authentication
    pub fn start(&mut self) {
        loop {
            //Get incoming client messages
            for mut msg in self.network_server.get_messages() {
                trace!("Received message {:?}", msg);
                //Check if user wishes to login
                if msg.field_type == Request_RequestType::AUTHENTICATE {
                    match msg.detailed_type {
                        Some(Request_oneof_detailed_type::auth(
                            Request_AuthenticateType::LOGIN,
                        )) => {
                            trace!("User is logging in..");
                            if let Err(mut e) = self.login(&msg) {
                                trace!(
                                    "Failed to login client ({}) with error ({})",
                                    msg.client_id,
                                    e
                                );
                                if e.process(self, msg.client_id) {
                                    return;
                                }
                            };
                            continue;
                        }
                        Some(Request_oneof_detailed_type::auth(
                            Request_AuthenticateType::NEW_USER,
                        )) => {
                            trace!("User is creating new account");
                            if let Err(mut e) = self.create_user(&msg) {
                                trace!(
                                    "Failed to login client ({}) with error ({})",
                                    msg.client_id,
                                    e
                                );
                                if e.process(self, msg.client_id) {
                                    return;
                                }
                            };
                            continue;
                        }
                        Some(_) | None => {}
                    }
                }
                if let Err(e) = self.token_is_valid(&mut msg) {
                    trace!(
                        "Failed to verify auth for '{}' error ({})",
                        msg.client_id,
                        e
                    );
                    if let Err(mut e) = self.send_login_request(&msg) {
                        if e.process(self, msg.client_id) {
                            return;
                        }
                    }
                } else {
                    if let Err(mut e) = self.parse_request(&msg) {
                        warn!("Error encountered in processing request ({}) ", e);
                        if e.process(self, msg.client_id) {
                            error!("Critical error encountered, must quit! ({})", e);
                            return;
                        }
                    }
                }
            }
        }
    }

    /// This executes the given request
    fn parse_request(&mut self, msg: &Request) -> Result<(), RequestError> {
        trace!("Processing request");
        match msg.field_type {
            Request_RequestType::SHUTDOWN => unimplemented!(),
            Request_RequestType::AUTHENTICATE => unimplemented!(),
            Request_RequestType::TRANSACTIONS => unimplemented!(),
            Request_RequestType::ACCOUNT => match msg.detailed_type {
                Some(Request_oneof_detailed_type::account(Request_AccountType::LIST_ACCOUNTS)) => {
                    trace!("Starting get all accounts");
                    return self.get_all_accounts(&msg);
                }
                Some(Request_oneof_detailed_type::account(Request_AccountType::ACCOUNT_INFO)) => {
                    trace!("Starting get detailed info");
                    return self.get_account_info(&msg);
                }

                Some(Request_oneof_detailed_type::account(Request_AccountType::NEW_ACCOUNT)) => {
                    trace!("Starting new account creation");
                    return self.new_account(msg);
                }

                Some(_) | None => info!("Has not matched any functions"),
            },
            Request_RequestType::MISC => unimplemented!(),
            Request_RequestType::Result => unimplemented!(),
        }
        warn!("Nothing was executed");
        Ok(())
    }
    fn send_incorrect_arguments_error(&mut self, client_id: String) -> Result<(), RequestError> {
        let request = Request::success_result(
            Vec::new(),
            client_id.clone(),
            Request_ResultType::INVALID_ARGS,
        );
        if self.network_server.send_message(request).is_err() {
            Err(RequestError::new(ErrorKind::NetworkFailure))
        } else {
            Ok(())
        }
    }
    fn send_authentication_error(&mut self, client_id: String) -> Result<(), RequestError> {
        let request = Request::success_result(
            Vec::new(),
            client_id.clone(),
            Request_ResultType::NOT_AUTHENTICATED,
        );
        if self.network_server.send_message(request).is_err() {
            Err(RequestError::new(ErrorKind::NetworkFailure))
        } else {
            Ok(())
        }
    }

    fn send_critical_error(&mut self, client_id: String) -> Result<(), RequestError> {
        let request =
            Request::success_result(Vec::new(), client_id, Request_ResultType::UNEXPECTED_ERROR);
        if self.network_server.send_message(request).is_err() {
            Err(RequestError::new(ErrorKind::NetworkFailure))
        } else {
            Ok(())
        }
    }

    fn send_login_request(&mut self, msg: &Request) -> Result<(), RequestError> {
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
        if self.network_server.send_message(auth_req).is_err() {
            Err(RequestError::new(ErrorKind::NetworkFailure))
        } else {
            Ok(())
        }
    }

    fn create_user(&mut self, msg: &Request) -> Result<(), RequestError> {
        info!("Creating user");
        let user_string = msg
            .data
            .get(0)
            .ok_or(RequestError::new(ErrorKind::InvalidArguments))?;

        let mut user: User = serde_json::from_str(user_string).unwrap();
        user.user_uuid = database_handler::new_secure_uuid_v4();
        user.join_date = chrono::Utc::today().naive_utc();
        user.archived = false;
        user.password = database_handler::DbConnection::hash_password(user.password);
        self.database_connection.new_user_account(&user)?;
        let success_request = Request::success_result(
            Vec::new(),
            String::from(&msg.client_id),
            Request_ResultType::SUCCESS,
        );
        self.network_server
            .send_message(success_request)
            .map_err(|e| RequestError::new(ErrorKind::NetworkFailure))?;
        info!("Succesfully created user");
        Ok(())
    }

    fn login(&mut self, msg: &Request) -> Result<(), RequestError> {
        trace!("Logging in...");
        let token_req = self.database_connection.login(
            &String::from(
                msg.data
                    .get(0)
                    .ok_or(RequestError::new(ErrorKind::InvalidArguments))?,
            ),
            &String::from(
                msg.data
                    .get(1)
                    .ok_or(RequestError::new(ErrorKind::InvalidArguments))?,
            ),
        );

        let result: Request = match token_req {
            Ok(token) => {
                info!("Succesfully logged in");
                let mut data = Vec::new();
                data.push(token);
                Request::success_result(data, msg.client_id.clone(), Request_ResultType::SUCCESS)
            }

            Err(_) => {
                info!("Invalid parameters for logging in");
                Request::success_result(
                    Vec::new(),
                    msg.client_id.clone(),
                    Request_ResultType::INVALID_ARGS,
                )
            }
        };
        self.network_server
            .send_message(result)
            .map_err(|e| RequestError::new(ErrorKind::NetworkFailure))?;

        Ok(())
    }

    /// Validates a token given by the user
    ///
    /// If it is valid, it will set the user_uuid on the request
    /// to the matching user_uuid of the token
    ///
    /// #Return
    ///     Ok(True) - The token is valid
    ///     Ok(False) -  The token has expired/invalid
    ///     Err - The token doesn't exist/unexpected error
    fn token_is_valid(&mut self, msg: &mut Request) -> Result<(), RequestError> {
        return match self.database_connection.check_token(msg.token_id.clone()) {
            Ok(uuid) => {
                msg.user_id = uuid;
                Ok(())
            }
            Err(e) => Err(RequestError::new(ErrorKind::InvalidArguments)),
        };
    }
    fn get_account_info(&mut self, msg: &Request) -> Result<(), RequestError> {
        info!(
            "Retrieving account info for account ({:?}) and user {:?}",
            msg.data
                .get(0)
                .ok_or(RequestError::new(ErrorKind::InvalidArguments))?,
            msg.user_id
        );
        let account = self.database_connection.get_bank_account(
            msg.data[0]
                .parse()
                .map_err(|_e| RequestError::new(ErrorKind::InvalidArguments))?,
            msg.user_id
                .parse()
                .map_err(|_e| RequestError::new(ErrorKind::InvalidArguments))?,
        )?;
        let mut data: Vec<String> = Vec::new();
        data.push(
            serde_json::to_string(&account)
                .map_err(|_e| RequestError::new(ErrorKind::InvalidArguments))?,
        );
        self.network_server
            .send_message(Request::success_result(
                data,
                msg.client_id.clone(),
                Request_ResultType::SUCCESS,
            ))
            .map_err(|_e| RequestError::new(ErrorKind::NetworkFailure))
    }

    fn get_all_accounts(&mut self, msg: &Request) -> Result<(), RequestError> {
        info!("Listing accounts for user {:?}", msg.user_id);
        let user_uuid = msg
            .user_id
            .parse()
            .map_err(|e| RequestError::new(ErrorKind::InvalidArguments))?;
        let accounts = self.database_connection.get_all_bank_accounts(user_uuid)?;
        let account_str = serde_json::to_string(&accounts).unwrap();
        trace!(
            "Retrieved {} accounts, producing a json of {}",
            accounts.len(),
            &account_str
        );
        let mut data = Vec::new();
        data.push(account_str);
        self.network_server
            .send_message(Request::success_result(
                data,
                msg.client_id.clone(),
                Request_ResultType::SUCCESS,
            ))
            .map_err(|e| RequestError::new(ErrorKind::NetworkFailure))?;
        Ok(())
    }
    fn new_account(&mut self, msg: &Request) -> Result<(), RequestError> {
        info!("Creating account for user {:?}", msg.user_id);
        /*let account_str = msg
            .data
            .get(0)
            .ok_or(RequestError::new(ErrorKind::InvalidArguments))?;
        */
        warn!("Creating dummy 'simple_account'!");
        let mut account: Account = Account::simple_account();
        account.user_uuid = msg
            .user_id
            .parse()
            .map_err(|_e| RequestError::new(ErrorKind::NotAuthenticated))?;
        self.database_connection.new_bank_account(&account)?;
        self.network_server
            .send_message(Request::success_result(
                Vec::new(),
                String::from(&msg.client_id),
                Request_ResultType::SUCCESS,
            ))
            .map_err(|e| RequestError::new(ErrorKind::NetworkFailure))?;
        Ok(())
    }
}
