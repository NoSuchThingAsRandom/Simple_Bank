#[macro_use]
extern crate diesel;
extern crate argon2;
extern crate base64;
extern crate dotenv;
extern crate rand;
extern crate rand_chacha;

pub mod models;
pub mod schema;

use crate::models::{Account, Token, User};

use dotenv::dotenv;
use std::{env, fmt};

use log::warn;
use rand::{RngCore, SeedableRng};

use chrono::Duration;
use uuid::{Builder, Uuid, Variant, Version};

use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error;
use serde::export::Formatter;
use std::convert::TryInto;

//TODO Is it worth requiring a token for every request (In the database)
//TODO Even though it should be authenticated from load_balancer?

/// Holder for a database connection
pub struct DbConnection {
    connection: PgConnection,
}

pub struct DatabaseError {
    pub error_kind: DatabaseErrorKind,
}
impl DatabaseError {
    fn new(errorKind: DatabaseErrorKind) -> DatabaseError {
        DatabaseError {
            error_kind: errorKind,
        }
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
        }
    }
}
impl fmt::Debug for DatabaseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}
impl std::error::Error for DatabaseError {}
pub enum DatabaseErrorKind {
    AuthenticationError,
    NotFound,
    AlreadyExists,
    DatabaseFailure,
    Unknown,
}
impl DbConnection {
    /// Opens a new connection to the database
    ///
    /// The address is given by the .env file
    ///
    /// #Example
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let mut connection = DbConnection::new_connection();
    /// ```
    /// #Panics
    ///     .env file is invalid
    ///     $DATABASE_URL is not set
    pub fn new_connection() -> DbConnection {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let connection = PgConnection::establish(&database_url)
            .expect(&format!("Failed connecting to DB {}", database_url));
        DbConnection { connection }
    }

    //TODO Implement close function
    pub fn close(&mut self) {}
    /// Inserts a new user account to the database, using the given struct
    ///
    /// #Example
    /// ```
    /// use database_handler::models::User;
    /// use database_handler::{new_secure_uuid_v4, DbConnection};
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
    ///
    /// #Error
    ///     Error(diesel::result::Error::NotFound) - Failed due to collisions
    ///     Error(E) - Failed
    pub fn new_user_account(&mut self, details: &User) -> Result<(), DatabaseError> {
        DbConnection::check_query_processed(
            diesel::insert_into(schema::user_details::table)
                .values(details)
                .execute(&self.connection),
        )
    }
    /// Inserts a new banck account to the database, using the given struct
    ///
    /// #Example
    /// ```
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
    /// #Error
    ///     Error(diesel::result::Error::NotFound) - Failed due to collisions
    ///     Error(E) - Failed
    pub fn new_bank_account(&mut self, details: &Account) -> Result<(), DatabaseError> {
        DbConnection::check_query_processed(
            diesel::insert_into(schema::bank_accounts::table)
                .values(details)
                .execute(&self.connection),
        )
    }
    //TODO Switch to token
    /// Retrieves a user_uuid from a username
    /// #Example
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let username =String::from("some_uuid");
    /// let mut connection = DbConnection::new_connection();
    ///
    /// let user = connection.get_user_account(user_uuid).unwrap();
    /// ```
    /// #Error
    ///     Err(NotFound) - No user with that uuid found
    pub fn get_user_uuid(&mut self, username: &String) -> QueryResult<Uuid> {
        schema::user_details::table
            .filter(schema::user_details::username.eq(username))
            .select(schema::user_details::user_uuid)
            .first(&self.connection)
    }

    /// Retrieves user account details from a uuid
    ///
    /// #Example
    /// ```
    /// use database_handler::DbConnection;
    ///
    /// let user_uuid: uuid::Uuid ="some_uuid".parse().unwrap();
    /// let mut connection = DbConnection::new_connection();
    ///
    /// let user = connection.get_user_account(user_uuid).unwrap();
    /// ```
    /// #Error
    ///     Err(NotFound) - No user with that uuid found
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
    /// #Example
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
    /// #Error
    ///     Err(NotFound) - No account with that number found
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

    /// Sets the archive flag on a user account, so that it can no longer be modified
    ///
    /// Only the user that owns the account can modify the archive flag
    /// #Example
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
    /// #Example
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
    /// Helper function for ensuring that a record is updated
    fn check_query_processed(query: QueryResult<usize>) -> Result<(), DatabaseError> {
        if let Ok(rows) = query {
            return if rows > 0 {
                Ok(())
            } else {
                Err(DatabaseError::new(DatabaseErrorKind::AlreadyExists))
            };
        }
        Err(DatabaseError::new(DatabaseErrorKind::DatabaseFailure))
    }
    /// Checks if a given token is valid for the given user
    /// And returns the user_uuid for the owner of the token
    /// #Example
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
    /// let user_uuid = connection.check_token(token);    
    /// ```
    /// #Returns
    /// Ok(True) - The token is valid
    ///
    /// Ok(False) -  The token has expired/invalid
    ///
    /// #Error
    /// Err - The token doesn't exist/unexpected error
    pub fn check_token(&mut self, token_id: String) -> Result<String, DatabaseError> {
        let token: Token = if let Ok(_token) = schema::tokens::table
            .filter(schema::tokens::token.eq(token_id))
            .first(&self.connection)
        {
            _token
        } else {
            return Err(DatabaseError::new(DatabaseErrorKind::NotFound));
        };

        if chrono::Utc::now().naive_utc().ge(&token.expiry_date) {
            return Err(DatabaseError::new(DatabaseErrorKind::AuthenticationError));
        }
        if token.start_date.ge(&token.expiry_date) {
            //TODO This should never happen, is it worth checking?
            warn!("This should never happen, is it worth checking?");
            return Err(DatabaseError::new(DatabaseErrorKind::AuthenticationError));
        }
        if (token.start_date + chrono::Duration::weeks(52)).ge(&token.expiry_date) {
            //TODO Limits token duration to one year. May not be necessary?
            warn!("Limits token duration to one year. May not be necessary?");
            return Err(DatabaseError::new(DatabaseErrorKind::AuthenticationError));
        }
        Ok(token.client_uuid.to_string())
    }
    /// Creates a new token
    ///
    /// Uses rand chacha with 20 rounds
    ///
    /// Token is 32 bytes in length and base64 encodes
    /// #Example
    /// ```
    /// let token: String = generate_token();
    /// ```
    fn generate_token() -> String {
        let mut bytes: [u8; 32] = [0; 32];
        let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
        rng.fill_bytes(&mut bytes);
        base64::encode(bytes)
    }

    /// Updates a user's password
    ///
    /// Sets the password of the user with the given uuid, to a hashed version of the password
    /// Requires a valid token for that user to be executed
    ///
    ///#Example
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

    /// Checks the username and password are correct, and returns a token valid for 30 minutes
    ///
    ///
    /// #Example
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
            let rows = diesel::insert_into(schema::tokens::table)
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

    /// Hashes a password using argon2
    ///
    /// #Example
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
}

/// Generates a new unique uuid
/// #Example
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
    use crate::models::User;
    use crate::{new_secure_uuid_v4, DbConnection};
    use chrono::Utc;

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
