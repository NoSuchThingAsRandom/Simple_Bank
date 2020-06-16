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

use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use log::{error, trace, warn};

use rand::{RngCore, SeedableRng};
use std::env;

use chrono::Duration;
use uuid::{Builder, Uuid, Variant, Version};

pub struct DbConnection {
    connection: PgConnection,
}

impl DbConnection {
    pub fn new_connection() -> DbConnection {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let connection = PgConnection::establish(&database_url)
            .expect(&format!("Failed connecting to DB {}", database_url));
        DbConnection { connection }
    }
    pub fn new_user_account(&mut self, details: &User) -> QueryResult<()> {
        DbConnection::check_query_processed(
            diesel::insert_into(schema::user_details::table)
                .values(details)
                .execute(&self.connection),
        )
    }
    pub fn new_bank_account(&mut self, details: Account) -> QueryResult<()> {
        DbConnection::check_query_processed(
            diesel::insert_into(schema::bank_accounts::table)
                .values(&details)
                .execute(&self.connection),
        )
    }
    pub fn get_user_account(&mut self, user_uuid: uuid::Uuid) -> QueryResult<User> {
        let res = schema::user_details::table
            .find(user_uuid)
            .get_result(&self.connection);
        if res.is_err() {}
        res
    }
    pub fn get_bank_account(&mut self, account_number: i32) -> QueryResult<Account> {
        schema::bank_accounts::table
            .find(account_number)
            .get_result(&self.connection)
    }
    pub fn archive_user_account(&mut self, user_uuid: Uuid) -> QueryResult<()> {
        let user: User = schema::user_details::table
            .filter(schema::user_details::user_uuid.eq(user_uuid))
            .first(&self.connection)?;
        if user.archived {
            warn!("User has already been archived! ");
            return Err(diesel::result::Error::NotFound);
        }
        DbConnection::check_query_processed(
            diesel::update(
                schema::user_details::table.filter(schema::user_details::user_uuid.eq(user_uuid)),
            )
            .set(schema::user_details::archived.eq(true))
            .execute(&self.connection),
        )
    }
    pub fn delete_user_account(&mut self, user_uuid: Uuid) -> QueryResult<()> {
        DbConnection::check_query_processed(
            diesel::delete(
                schema::user_details::table.filter(schema::user_details::user_uuid.eq(user_uuid)),
            )
            .execute(&self.connection),
        )
    }
    fn get_user_uuid(&mut self, username: String) -> QueryResult<Uuid> {
        schema::user_details::table
            .filter(schema::user_details::username.eq(username))
            .select(schema::user_details::user_uuid)
            .first(&self.connection)
    }
    fn check_query_processed(query: QueryResult<usize>) -> QueryResult<()> {
        if let Ok(rows) = query {
            if rows > 0 {
                return Ok(());
            } else {
                panic!("No rows updated")
            }
        }
        Err(query.err().expect("Failed to unwrap error"))
    }
    /** Checks if a given token is valid for the given user
        Ok(True) - The token is valid
        Ok(False) -  The token has expired/invalid
        Err - The token doesn't exist/unexpected error
    **/
    pub fn check_token(&mut self, token_id: String, client_id: Uuid) -> QueryResult<bool> {
        let token: Token = schema::tokens::table
            .filter(schema::tokens::token.eq(token_id))
            .first(&self.connection)?;
        if chrono::Utc::now().naive_utc().ge(&token.expiry_date) {
            return Ok(false);
        }
        if token.start_date.ge(&token.expiry_date) {
            //TODO This should never happen, is it worth checking?
            warn!("This should never happen, is it worth checking?");
            return Ok(false);
        }
        if (token.start_date + chrono::Duration::weeks(52)).ge(&token.expiry_date) {
            //TODO Limits token duration to one year. May not be necessary?
            warn!("Limits token duration to one year. May not be necessary?");
            return Ok(false);
        }
        if !client_id.eq(&token.client_uuid) {
            return Ok(false);
        }
        return Ok(true);
    }
    fn generate_token() -> String {
        let mut bytes: [u8; 32] = [0; 32];
        let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
        rng.fill_bytes(&mut bytes);
        base64::encode(bytes)
    }

    pub fn update_password(&mut self, user_uuid: Uuid, password: String) -> QueryResult<bool> {
        let hash = DbConnection::hash_password(password);
        let result = diesel::update(
            schema::user_details::table.filter(schema::user_details::user_uuid.eq(user_uuid)),
        )
        .set(schema::user_details::password.eq(hash))
        .execute(&self.connection);
        return if let Ok(size) = result {
            if size == 1 {
                Ok(true)
            } else if size > 1 {
                //TODO Remove panic
                panic!("Updated more than one password!");
            } else {
                Ok(false)
            }
        } else {
            Err(result.err().unwrap())
        };
    }

    /**
        If the username and password are correct, returns a Ok(Some(authentication key))
        If the username and password are incorrect retunrs Ok(None)
        Otherwise returns the error encountered
    **/
    pub fn login(&mut self, username: String, password: String) -> QueryResult<Option<String>> {
        let hashed: Result<String, diesel::result::Error> = schema::user_details::table
            .filter(schema::user_details::username.eq(&username))
            .select(schema::user_details::password)
            .first(&self.connection);
        if let Err(e) = hashed {
            if e == diesel::result::Error::NotFound {
                trace!("Invalid username");
                return Ok(None);
            }
            error!("Unexpected error logging in {}", e);
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
                trace!("Created token");
                Ok(Some(token))
            } else {
                trace!("Failed to create token");
                Ok(None)
            }
        } else {
            trace!("Invalid password");
            Ok(None)
        }
    }

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

/** Generates a new unique uuid
    Uses secure random

**/
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
    use log::LevelFilter;
    use log::{error, trace, warn};
    use simplelog::{ConfigBuilder, TermLogger, TerminalMode};
    use std::str::FromStr;

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
        trace!("Starting l");
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
        assert!(con.delete_user_account(user.user_uuid).is_ok());
    }

    #[test]
    fn check_auth_token() {
        let user = get_testing_user();
        let username = user.username.clone();
        let password = user.password.clone();
        check_user_exists(&user);
        let mut con = DbConnection::new_connection();
        let token_res = con.login(username, password);
        assert!(token_res.is_ok());
        let token_opt = token_res.unwrap();
        assert!(token_opt.is_some());
        let token = token_opt.unwrap();
        let check_token = con.check_token(token, user.user_uuid);
        assert!(check_token.is_ok());
        assert!(check_token.unwrap());
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
        con.new_user_account(&user);
        assert!(con.archive_user_account(user_id).is_ok());
        con.delete_user_account(user_id).unwrap();
    }
}
