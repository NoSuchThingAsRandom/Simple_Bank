extern crate argon2;
extern crate base64;
extern crate diesel;
extern crate dotenv;
extern crate rand;
extern crate rand_chacha;
extern crate structs;

use dotenv::dotenv;
use std::{env, fmt};
use structs::models::{Account, Token, Transaction, User};
use structs::schema;

use log::{error, info, trace, warn};
use rand::{RngCore, SeedableRng};

use chrono::Duration;
use uuid::{Builder, Uuid, Variant, Version};

use diesel::pg::PgConnection;
use diesel::prelude::*;
use serde::export::Formatter;
use std::io::ErrorKind;

//TODO Is it worth requiring a token for every request (In the database)
//TODO Even though it should be authenticated from load_balancer?

/// Wrapper over the diesel crate for connecting to and issuing requests to a postgres database
pub struct DbConnection {
    connection: PgConnection,
}

pub struct DatabaseError {
    pub error_kind: DatabaseErrorKind,
}

impl DatabaseError {
    ///Creates a new DatabaseError with the given error kind
    fn new(error_kind: DatabaseErrorKind) -> DatabaseError {
        DatabaseError { error_kind }
    }
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.error_kind {
            DatabaseErrorKind::AuthenticationError => {
                write!(f, "The request was not authenticated properly!")
            }
            DatabaseErrorKind::AlreadyExists => {
                write!(f, "A record with those unique fields already exists!")
            }
            DatabaseErrorKind::DatabaseFailure => write!(
                f,
                "An unexpected error occured with the database connection"
            ),
            DatabaseErrorKind::Unknown => write!(f, "Unknown failure"),
            DatabaseErrorKind::NotFound => write!(f, "The record was not found!"),
            DatabaseErrorKind::InvalidArguments => write!(
                f,
                "Invalid arguments were provided (Could be account does not have enough money",
            ),
        }
    }
}

impl fmt::Debug for DatabaseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error occurred!!!")
    }
}

impl std::error::Error for DatabaseError {}

///Container for identifying error types
pub enum DatabaseErrorKind {
    AuthenticationError,
    NotFound,
    AlreadyExists,
    DatabaseFailure,
    InvalidArguments,
    Unknown,
}

impl DbConnection {
    /// Opens a new connection to the database
    ///
    /// The address is given by the enviroment variable $DATABASE_URL which must be set by a .env file
    ///
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let mut connection = DbConnection::new_connection();
    /// ```
    /// # Panics
    ///     Panics if the .env file is invalid/not found
    ///     Panics if the $DATABASE_URL is not set
    pub fn new_connection() -> DbConnection {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let connection = PgConnection::establish(&database_url)
            .expect(&format!("Failed connecting to DB {}", database_url));
        DbConnection { connection }
    }

    /// Creates a new token
    ///
    /// Uses rand chacha with 20 rounds
    ///
    /// Token is 32 bytes in length and base64 encodes
    /// # Examples
    /// ```
    /// let token: String = generate_token();
    /// ```
    fn generate_token() -> String {
        let mut bytes: [u8; 32] = [0; 32];
        let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
        rng.fill_bytes(&mut bytes);
        base64::encode(bytes)
    }

    /// Hashes the given string using the argon2 crate
    ///
    /// Will return a string with the hashed password in a compatible argon2 format
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    /// let password = String::from("password");
    /// let hashed = DbConnection::hash_password(password);
    /// ```
    pub fn hash_password(password: String) -> String {
        let b_password = password.as_bytes();
        let mut salt: [u8; 16] = [0; 16];
        let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
        rng.fill_bytes(&mut salt);
        let config = argon2::Config::default();
        let hash = argon2::hash_encoded(b_password, &salt, &config).unwrap();
        return hash;
    }

    /// Helper function for ensuring that a record is updated
    fn check_query_processed(query: QueryResult<usize>) -> Result<(), DatabaseError> {
        match query {
            Ok(rows) => {
                if rows > 0 {
                    trace!("Successfully updated {} row", rows);
                    Ok(())
                } else {
                    warn!("No records altered!");
                    Err(DatabaseError::new(DatabaseErrorKind::AlreadyExists))
                }
            }
            Err(e) => {
                if let diesel::result::Error::DatabaseError(error_kind, _) = e {
                    match error_kind {
                        diesel::result::DatabaseErrorKind::UniqueViolation => {
                            warn!("Record already exists!");
                            Err(DatabaseError::new(DatabaseErrorKind::AlreadyExists))
                        }
                        _ => {
                            error!("Database failure? {:?}", e);
                            Err(DatabaseError::new(DatabaseErrorKind::DatabaseFailure))
                        }
                    }
                } else {
                    error!("Database failure? {:?}", e);
                    Err(DatabaseError::new(DatabaseErrorKind::DatabaseFailure))
                }
            }
        }
    }

    /// Checks if a given token is valid for the given user
    ///
    /// And returns the user_uuid in string format for the owner of the token
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let username=String::from("username");
    /// let password=String::from("password");
    ///
    /// let mut connection = DbConnection::new_connection();
    /// let token = connection.login(&username,&password).unwrap().unwrap();
    ///
    /// let user_uuid = match connection.check_token(token){
    ///     Ok(token)=>token,
    ///     Err(e)=>{
    ///         println!("Invalid token provided with error ({})",e);
    ///     }
    /// };    
    /// ```
    ///
    pub fn check_token(&mut self, token_id: String) -> Result<String, DatabaseError> {
        info!("Checking if token '{}' is valid", token_id);
        let token: Token = if let Ok(token) = schema::tokens::table
            .filter(schema::tokens::token.eq(token_id))
            .first(&self.connection)
        {
            token
        } else {
            trace!("Didn't find matching token in database");
            return Err(DatabaseError::new(DatabaseErrorKind::NotFound));
        };

        if chrono::Utc::now().naive_utc().ge(&token.expiry_date) {
            trace!("Token date has expired");
            return Err(DatabaseError::new(DatabaseErrorKind::AuthenticationError));
        }
        if token.start_date.ge(&token.expiry_date) {
            //TODO This should never happen, is it worth checking?
            warn!("Token start date is before expire data");
            return Err(DatabaseError::new(DatabaseErrorKind::AuthenticationError));
        }
        if (token.start_date + chrono::Duration::weeks(52)).le(&token.expiry_date) {
            //TODO Limits token duration to one year. May not be necessary?
            warn!("Token is older than a year! May not be necessary?");
            return Err(DatabaseError::new(DatabaseErrorKind::AuthenticationError));
        }
        trace!("Token is valid");
        Ok(token.client_uuid.to_string())
    }

    /// Inserts a new user account to the database
    ///
    /// # Examples
    /// ```
    /// use database_handler::models::User;
    /// use database_handler::{new_secure_uuid_v4, DbConnection};
    /// use structs::models::User;
    /// let user:User =User{
    ///     user_uuid:new_secure_uuid_v4(),
    ///     username:String::from("username"),
    ///     password:String::from("password"),
    ///     email:String::from("username@company.com"),
    ///     date_of_birth:chrono::NaiveDate::from_ymd(1990,1,1),
    ///     join_date:chrono::Utc::today().naive_utc(),
    ///     archived:false
    /// };
    ///
    /// let mut connection = DbConnection::new_connection();
    /// match connection.new_user_account(&user){
    ///     Ok(_)=>println!("Added to database"),
    ///     Err(e)=>{
    ///         if e==diesel::NotFound{
    ///             println!("Collision with existing account")
    ///         } else {
    ///             println!("Failure adding to database")
    ///         }
    ///     }
    /// }
    /// ```
    /// # Returns
    ///     Ok(()) - The account was inserted
    ///     Err(e) - The insertion failed
    pub fn new_user_account(&mut self, details: &User) -> Result<(), DatabaseError> {
        trace!("Inserting new user account");
        DbConnection::check_query_processed(
            diesel::insert_into(schema::user_details::table)
                .values(details)
                .execute(&self.connection),
        )
    }

    /// Inserts a new bank account to the database
    ///
    /// # Examples
    /// ```
    /// extern crate structs;
    /// use structs::models::Account;
    /// use database_handler::models::Account;
    /// use database_handler::{new_secure_uuid_v4, DbConnection};
    ///
    /// let user_uuid=new_secure_uuid_v4();
    /// let account:Account =Account{
    ///     account_number:12345678,
    ///     user_uuid,
    ///     balance:bigdecimal::BigDecimal::from(0),
    ///     interest_rate:bigdecimal::BigDecimal::from(0),
    ///     sort_code:123456,
    ///     overdraft_limit:bigdecimal::BigDecimal::from(0),
    ///     account_name:Some(String::from("Easy_Access_Saver")),
    ///     account_category:Some(String::from("Savings")),
    ///     archived:false
    /// };
    ///
    /// let mut connection = DbConnection::new_connection();
    /// match connection.new_bank_account(&account){
    ///     Ok(_)=>println!("Added to database"),
    ///     Err(e)=>{
    ///         if e==diesel::NotFound{
    ///             println!("Collision with existing account")
    ///         } else {
    ///             println!("Failure adding to database")
    ///         }
    ///     }
    /// }
    ///
    /// ```
    ///
    /// # Returns
    ///     Ok(()) - The account was inserted
    ///     Err(e) - The insertion failed
    pub fn new_bank_account(&mut self, details: &Account) -> Result<(), DatabaseError> {
        DbConnection::check_query_processed(
            diesel::insert_into(schema::bank_accounts::table)
                .values(details)
                .execute(&self.connection),
        )
    }

    /// Checks the username and password are correct, and returns a token valid for 30 minutes
    ///
    ///
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let username=String::from("username");
    /// let password=String::from("password");
    ///
    /// let mut connection = DbConnection::new_connection();
    ///
    /// let token = connection.login(&username,&password).unwrap().unwrap();
    /// ```
    /// #Errors
    ///     Err(NotFound) - The username or password don't match
    pub fn login(&mut self, username: &String, password: &String) -> QueryResult<String> {
        let hashed: Result<String, diesel::result::Error> = schema::user_details::table
            .filter(schema::user_details::username.eq(&username))
            .select(schema::user_details::password)
            .first(&self.connection);
        if let Err(e) = hashed {
            if e == diesel::result::Error::NotFound {
                return Err(diesel::NotFound);
            }
            warn!("Unexpected error logging in {}", e);
            return Err(e);
        }
        if argon2::verify_encoded(&hashed.unwrap(), password.as_bytes()).unwrap() {
            let token = DbConnection::generate_token();
            let client_uuid = self.get_user_uuid(username).unwrap();
            let current_time = chrono::Utc::now().naive_utc();
            let end_time = current_time + Duration::minutes(30);
            let insert_token: Token = Token {
                token: token.clone(),
                client_uuid,
                start_date: current_time,
                expiry_date: end_time,
            };
            trace!("Valid username,password creating token");
            let rows = diesel::insert_into(structs::schema::tokens::table)
                .values(insert_token)
                .execute(&self.connection);
            if rows == Ok(1) {
                Ok(token)
            } else {
                panic!("Created more than one token");
            }
        } else {
            return Err(diesel::NotFound);
        }
    }

    /// Updates a user's password
    ///
    /// Sets the password of the user with the given uuid, to a hashed version of the password
    /// Requires a valid token for that user to be executed
    ///
    ///# Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let username=String::from("username");
    /// let old_password=String::from("password");
    /// let new_password=String::from("password");
    ///
    /// let mut connection = DbConnection::new_connection();
    /// let user_uuid: uuid::Uuid = connection.get_user_uuid(&username).unwrap();
    /// let token = connection.login(&username,&old_password).unwrap().unwrap();
    ///
    /// connection.update_password(token,user_uuid,new_password);
    /// ```
    ///
    /// #Error
    ///     Err(NotFound) - Could not find that user in the database
    pub fn update_password(
        &mut self,
        token: String,
        user_uuid: Uuid,
        password: String,
    ) -> Result<bool, DatabaseError> {
        //Checks for authentication
        let token_user_uuid: String = self.check_token(token)?;
        if !token_user_uuid.eq(&user_uuid.to_string()) {
            return Err(DatabaseError::new(DatabaseErrorKind::AuthenticationError));
        }

        let hash = DbConnection::hash_password(password);
        let result = diesel::update(
            schema::user_details::table.filter(schema::user_details::user_uuid.eq(user_uuid)),
        )
        .set(schema::user_details::password.eq(hash))
        .execute(&self.connection);
        if let Ok(size) = result {
            if size == 1 {
                Ok(true)
            } else if size > 1 {
                //TODO Remove panic
                panic!("Updated more than one password!");
            } else {
                Err(DatabaseError::new(DatabaseErrorKind::NotFound))
            }
        } else {
            Err(DatabaseError::new(DatabaseErrorKind::DatabaseFailure))
        }
    }

    //TODO Switch to token
    /// Retrieves a user_uuid from a username
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let username =String::from("some_uuid");
    /// let mut connection = DbConnection::new_connection();
    ///
    /// let user = connection.get_user_account(user_uuid).unwrap();
    /// ```
    /// # Error
    ///     Err(NotFound) - No user with that uuid found
    pub fn get_user_uuid(&mut self, username: &String) -> QueryResult<Uuid> {
        schema::user_details::table
            .filter(schema::user_details::username.eq(username))
            .select(schema::user_details::user_uuid)
            .first(&self.connection)
    }

    /// Retrieves user account details from a uuid
    ///
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let user_uuid: uuid::Uuid ="some_uuid".parse().unwrap();
    /// let mut connection = DbConnection::new_connection();
    ///
    /// let user = connection.get_user_account(user_uuid).unwrap();
    /// ```
    pub fn get_user_account(&mut self, user_uuid: uuid::Uuid) -> QueryResult<User> {
        schema::user_details::table
            .find(user_uuid)
            .get_result(&self.connection)
    }

    /// Retrieves bank account details
    ///
    /// Gets the bank account matching the given account number,
    /// AND the given user_uuid owning the account
    ///
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let user_uuid: uuid::Uuid ="some_uuid".parse().unwrap();
    /// let account_number: i32 = 12345678;
    ///
    /// let mut connection = DbConnection::new_connection();
    ///
    /// let user = connection.get_bank_account(account_number,user_uuid).unwrap();
    /// ```
    /// # Error
    ///     Err(NotFound) - No account with that number found
    ///     Err(DatabaseFailure) - An error occured with the database
    pub fn get_bank_account(
        &mut self,
        account_number: i32,
        user_uuid: Uuid,
    ) -> Result<Account, DatabaseError> {
        match schema::bank_accounts::table
            .find(account_number)
            .filter(schema::bank_accounts::user_uuid.eq(user_uuid))
            .get_result(&self.connection)
        {
            Ok(account) => Ok(account),
            Err(e) => {
                if e == diesel::result::Error::NotFound {
                    Err(DatabaseError::new(DatabaseErrorKind::NotFound))
                } else {
                    Err(DatabaseError::new(DatabaseErrorKind::DatabaseFailure))
                }
            }
        }
    }

    /// Retrieves bank account details
    ///
    /// Gets the bank account matching the given account number,
    /// AND the given user_uuid owning the account
    ///
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let user_uuid: uuid::Uuid ="some_uuid".parse().unwrap();
    /// let account_number: i32 = 12345678;
    ///
    /// let mut connection = DbConnection::new_connection();
    ///
    /// let user = connection.get_bank_account(account_number,user_uuid).unwrap();
    /// ```
    /// # Error
    ///     Err(NotFound) - No account with that number found
    ///     Err(DatabaseFailure) - An error occured with the database
    pub fn get_all_bank_accounts(
        &mut self,
        user_uuid: Uuid,
    ) -> Result<Vec<Account>, DatabaseError> {
        match schema::bank_accounts::table
            .filter(schema::bank_accounts::user_uuid.eq(user_uuid))
            .load::<Account>(&self.connection)
        {
            Ok(account) => Ok(account),
            Err(e) => {
                if e == diesel::result::Error::NotFound {
                    Err(DatabaseError::new(DatabaseErrorKind::NotFound))
                } else {
                    Err(DatabaseError::new(DatabaseErrorKind::DatabaseFailure))
                }
            }
        }
    }

    /// Finds a transaction by it's unique identifier
    ///
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let transaction_id: uuid::Uuid ="some_uuid".parse().unwrap();
    /// let mut connection = DbConnection::new_connection();
    ///
    /// let transasction = connection.get_transaction_by_uuid(transaction_id).unwrap();
    /// ```
    /// # Error
    ///     Err(NotFound) - No user with that uuid found
    ///     Err(DatabaseFailure) - An error occured with the database
    pub fn get_transaction_by_uuid(
        &mut self,
        transaction_id: uuid::Uuid,
    ) -> Result<Transaction, DatabaseError> {
        trace!("Retrieving transaction by id ({})", transaction_id);
        match schema::transactions::table
            .find(transaction_id)
            .get_result(&self.connection)
        {
            Ok(transaction) => Ok(transaction),
            Err(e) => {
                if e == diesel::result::Error::NotFound {
                    Err(DatabaseError::new(DatabaseErrorKind::NotFound))
                } else {
                    Err(DatabaseError::new(DatabaseErrorKind::DatabaseFailure))
                }
            }
        }
    }

    /// Finds all transactions executed by the given user
    ///
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let user_id: uuid::Uuid ="some_uuid".parse().unwrap();
    /// let mut connection = DbConnection::new_connection();
    ///
    /// let transactions = connection.get_transaction_by_uuid(user_id).unwrap();
    /// ```
    /// # Error
    ///     Err(NotFound) - No user with that uuid found
    ///     Err(DatabaseFailure) - An error occured with the database
    pub fn get_transaction_by_user(
        &mut self,
        user_uuid: uuid::Uuid,
    ) -> Result<Vec<Transaction>, DatabaseError> {
        trace!("Retrieving all transactions by user ({})", user_uuid);
        match schema::transactions::table
            .filter(schema::transactions::user_responsible.eq(user_uuid))
            .load::<Transaction>(&self.connection)
        {
            Ok(account) => Ok(account),
            Err(e) => {
                if e == diesel::result::Error::NotFound {
                    Err(DatabaseError::new(DatabaseErrorKind::NotFound))
                } else {
                    Err(DatabaseError::new(DatabaseErrorKind::DatabaseFailure))
                }
            }
        }
    }

    /// Finds all transactions for a given bank account
    ///
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let account_id = 123456;
    /// let mut connection = DbConnection::new_connection();
    ///
    /// let transactions = connection.get_transaction_by_account(account_id).unwrap();
    /// ```
    /// # Error
    ///     Err(NotFound) - No user with that uuid found
    ///     Err(DatabaseFailure) - An error occured with the database
    pub fn get_transaction_by_account(
        &mut self,
        account_id: i32,
    ) -> Result<Vec<Transaction>, DatabaseError> {
        trace!(
            "Retrieving all transactions by bank account ({})",
            account_id
        );
        match schema::transactions::table
            .filter(schema::transactions::source_account_number.eq(account_id))
            .or_filter(schema::transactions::dest_account_number.eq(account_id))
            .load::<Transaction>(&self.connection)
        {
            Ok(account) => Ok(account),
            Err(e) => {
                if e == diesel::result::Error::NotFound {
                    Err(DatabaseError::new(DatabaseErrorKind::NotFound))
                } else {
                    Err(DatabaseError::new(DatabaseErrorKind::DatabaseFailure))
                }
            }
        }
    }

    ///Transfers money between two internal accounts, if the user uuid matches the source account
    ///
    /// # Examples
    /// ```
    ///     use database_handler::DbConnection;
    ///     use std::str::FromStr;
    ///
    ///     let amount=bigdecimal::BigDecimal::from(5.0);
    ///     let source_account_id = 123456;
    ///     let source_user_uuid = uuid::Uuid::from_str(&"Some uuid").unwrap();
    ///     let dest_account_id = 123457;
    ///     
    ///     let mut connection =DbConnection::new_connection();
    ///     let result = connection.create_internal_transaction(source_account_id,dest_account_id,source_user_uuid,amount);
    ///     match result{
    ///         Ok(_) => println!("The transaction was completed successfully "),
    ///         Err(e)=> println!("Failed to create transaction, error ({}) " ,e)
    ///     }   
    ///     
    /// ```
    ///
    /// # Errors
    ///     Err(NotFound) => At least one of the accounts do not exist
    ///     Err(InvalidArguments) => There is not enough money in the source account
    pub fn create_internal_transaction(
        &mut self,
        source_account_number: i32,
        dest_account_number: i32,
        user_uuid: uuid::Uuid,
        transaction_amount: bigdecimal::BigDecimal,
    ) -> Result<(), DatabaseError> {
        trace!(
            "Transferring ({}) from account ({}) to account ({})",
            transaction_amount,
            source_account_number,
            dest_account_number
        );
        if transaction_amount < bigdecimal::BigDecimal::from(0) {
            return Err(DatabaseError::new(DatabaseErrorKind::InvalidArguments));
        }
        let source_sort_code: i32;
        let dest_sort_code: i32;
        let source_balance: bigdecimal::BigDecimal;
        let dest_balance: bigdecimal::BigDecimal;

        //Check source account exists, and matches user uuid
        match schema::bank_accounts::table
            .filter(schema::bank_accounts::account_number.eq(source_account_number))
            .filter(schema::bank_accounts::user_uuid.eq(user_uuid))
            .first::<Account>(&self.connection)
        {
            Ok(account) => {
                if &account.balance + &account.overdraft_limit < transaction_amount {
                    return Err(DatabaseError::new(DatabaseErrorKind::InvalidArguments));
                }
                source_balance = account.balance - &transaction_amount;
                source_sort_code = account.sort_code;
            }
            Err(_) => {
                return Err(DatabaseError::new(DatabaseErrorKind::NotFound));
            }
        }

        //Check destination account exists
        match schema::bank_accounts::table
            .filter(schema::bank_accounts::account_number.eq(dest_account_number))
            .first::<Account>(&self.connection)
        {
            Ok(account) => {
                dest_balance = account.balance + &transaction_amount;
                dest_sort_code = account.sort_code;
            }
            Err(_) => return Err(DatabaseError::new(DatabaseErrorKind::NotFound)),
        }
        trace!("Updating balances");
        //Update balances
        DbConnection::check_query_processed(
            diesel::update(
                schema::bank_accounts::table
                    .filter(schema::bank_accounts::account_number.eq(source_account_number)),
            )
            .set(schema::bank_accounts::balance.eq(source_balance))
            .execute(&self.connection),
        )?;
        DbConnection::check_query_processed(
            diesel::update(
                schema::bank_accounts::table
                    .filter(schema::bank_accounts::account_number.eq(dest_account_number)),
            )
            .set(schema::bank_accounts::balance.eq(dest_balance))
            .execute(&self.connection),
        )?;
        let transaction = Transaction {
            transaction_id: new_secure_uuid_v4(),
            user_responsible: user_uuid,
            time_issued: chrono::Utc::now().naive_utc(),
            amount: transaction_amount,
            source_sort_code,
            source_account_number,
            dest_sort_code,
            dest_account_number,
        };
        DbConnection::check_query_processed(
            diesel::insert_into(schema::transactions::table)
                .values(&transaction)
                .execute(&mut self.connection),
        )
    }

    /// Sets the archive flag on a user account, so that it can no longer be modified
    ///
    /// Only the user that owns the account can modify the archive flag
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let username=String::from("username");
    /// let password=String::from("password");
    ///
    /// let mut connection = DbConnection::new_connection();
    /// let user_uuid: uuid::Uuid = connection.get_user_uuid(&username).unwrap();
    /// let token = connection.login(&username,&password).unwrap().unwrap();
    ///
    /// let user = connection.archive_user_account(user_uuid,token).unwrap();
    /// ```
    /// #Error
    ///     DatabaseError -
    pub fn archive_user_account(
        &mut self,
        user_uuid: Uuid,
        token: String,
    ) -> Result<(), DatabaseError> {
        let token_user_uuid: String = self.check_token(token)?;
        if !token_user_uuid.eq(&user_uuid.to_string()) {
            return Err(DatabaseError::new(DatabaseErrorKind::AuthenticationError));
        }
        let user: User = if let Ok(_user) = schema::user_details::table
            .filter(schema::user_details::user_uuid.eq(user_uuid))
            .first(&self.connection)
        {
            _user
        } else {
            return Err(DatabaseError::new(DatabaseErrorKind::NotFound));
        };
        if user.archived {
            warn!("User has already been archived! ");
            return Err(DatabaseError::new(DatabaseErrorKind::AlreadyExists));
        }
        DbConnection::check_query_processed(
            diesel::update(
                schema::user_details::table.filter(schema::user_details::user_uuid.eq(user_uuid)),
            )
            .set(schema::user_details::archived.eq(true))
            .execute(&self.connection),
        )
    }

    /// Permantly removes the user account from the database
    ///
    /// Can only be done by the user that owns the account
    /// # Examples
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let username=String::from("username");
    /// let password=String::from("password");
    ///
    /// let mut connection = DbConnection::new_connection();
    /// let user_uuid: uuid::Uuid = connection.get_user_uuid(&username).unwrap();
    /// let token = connection.login(&username,&password).unwrap().unwrap();
    ///
    /// let user = connection.delete_user_account(user_uuid,token).unwrap();
    /// ```
    /// #Error
    ///     Err(NotFound) - No user with that uuid found
    pub fn delete_user_account(
        &mut self,
        user_uuid: Uuid,
        token: String,
    ) -> Result<(), DatabaseError> {
        let token_user_uuid: String = self.check_token(token)?;
        if !token_user_uuid.eq(&user_uuid.to_string()) {
            return Err(DatabaseError::new(DatabaseErrorKind::AuthenticationError));
        }
        DbConnection::check_query_processed(
            diesel::delete(
                schema::user_details::table.filter(schema::user_details::user_uuid.eq(user_uuid)),
            )
            .execute(&self.connection),
        )
    }

    //TODO Implement close function
    pub fn close(&mut self) {}
}

/// Generates a new unique uuid
/// # Examples
/// ```
/// use database_handler::new_secure_uuid_v4;
/// let uuid= new_secure_uuid_v4();
/// ```
pub fn new_secure_uuid_v4() -> Uuid {
    let mut bytes = [0; 16];
    let mut rng = rand_chacha::ChaCha20Rng::from_entropy();

    rng.fill_bytes(&mut bytes);

    Builder::from_bytes(bytes)
        .set_variant(Variant::RFC4122)
        .set_version(Version::Random)
        .build()
}

#[cfg(test)]
mod user_accounts_test {
    use crate::{new_secure_uuid_v4, DbConnection};
    use chrono::Utc;
    use structs::models::User;

    const USER_UUID_1: &str = "e78836e9-1982-4380-a678-a5b4db33d205";
    const USER_UUID_2: &str = "abe8e4bb-c87a-48ff-a4fb-2ebd42745aae";

    fn get_testing_user() -> User {
        let user_id = USER_UUID_2.parse().unwrap();
        User {
            user_uuid: user_id,
            username: "Test_02".to_string(),
            password: "password".to_string(),
            email: "test2@example.com".to_string(),
            date_of_birth: Utc::today().naive_utc(),
            join_date: Utc::today().naive_utc(),
            archived: false,
        }
    }

    fn check_user_exists(user: &User) {
        let mut con = DbConnection::new_connection();
        if con.get_user_account(user.user_uuid).is_err() {
            con.new_user_account(&user).unwrap();
        }
    }

    #[test]
    fn create_and_delete_user_account() {
        let user_id = USER_UUID_1.parse().unwrap();
        let user = User {
            user_uuid: user_id,
            username: "Test_01".to_string(),
            password: "password".to_string(),
            email: "test1@example.com".to_string(),
            date_of_birth: Utc::today().naive_utc(),
            join_date: Utc::today().naive_utc(),
            archived: false,
        };
        let mut con = DbConnection::new_connection();
        assert!(con.new_user_account(&user).is_ok());

        let token = con.login(&user.username, &user.password).unwrap();
        assert!(con.delete_user_account(user.user_uuid, token).is_ok());
    }

    #[test]
    fn check_auth_token() {
        let user = get_testing_user();
        let username = user.username.clone();
        let password = user.password.clone();
        check_user_exists(&user);
        let mut con = DbConnection::new_connection();
        let token_res = con.login(&username, &password);
        assert!(token_res.is_ok());
        let token = token_res.unwrap();
        let check_token = con.check_token(token);
        assert!(check_token.is_ok());
    }

    #[test]
    fn get_user_account() {
        let user = get_testing_user();
        let mut con = DbConnection::new_connection();
        check_user_exists(&user);
        assert!(con.get_user_account(user.user_uuid).is_ok());
    }

    #[test]
    fn archive_user_account() {
        let mut con = DbConnection::new_connection();
        let user_id = new_secure_uuid_v4();
        let user = User {
            user_uuid: user_id,
            username: "Test_03".to_string(),
            password: DbConnection::hash_password("password".to_string()),
            email: "test@example.com".to_string(),
            date_of_birth: Utc::today().naive_utc(),
            join_date: Utc::today().naive_utc(),
            archived: false,
        };
        con.new_user_account(&user).unwrap();
        let token = con.login(&user.username, &user.password).unwrap();
        assert!(con.archive_user_account(user_id, token.clone()).is_ok());
        con.delete_user_account(user_id, token.clone()).unwrap();
    }
}
