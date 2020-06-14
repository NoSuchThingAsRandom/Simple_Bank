#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate rand;

pub mod models;
pub mod schema;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use std::env;
use crate::models::{User, Account};
use std::error::Error;
use uuid::{Uuid, Builder, Variant, Version};
use log::error;
use rand::{SeedableRng, RngCore};
use chrono::Utc;
use rand::seq::index::IndexVec::U32;

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
    pub fn new_user_account(&mut self, details: User) -> QueryResult<()> {
        DbConnection::check_query_processed(diesel::insert_into(schema::user_details::table).values(&details).execute(&self.connection))
    }
    pub fn new_bank_account(&mut self, details: Account) -> QueryResult<()> {
        DbConnection::check_query_processed(diesel::insert_into(schema::bank_accounts::table).values(&details).execute(&self.connection))
    }
    pub fn get_user_account(&mut self, user_uuid: uuid::Uuid) -> QueryResult<User> {
        schema::user_details::table.find(user_uuid).get_result(&self.connection)
    }
    pub fn get_bank_account(&mut self, account_number: i32) -> QueryResult<Account> {
        schema::bank_accounts::table.find(account_number).get_result(&self.connection)
    }
    pub fn archive_user_account(&mut self, user_uuid: Uuid) -> QueryResult<()> {
        let user: User = schema::user_details::table.filter(schema::user_details::user_uuid.eq(user_uuid)).first(&self.connection)?;
        if user.archived {
            println!("User has already been archived! ");
            return Err(diesel::result::Error::NotFound);
        }
        DbConnection::check_query_processed(diesel::update(schema::user_details::table.filter(schema::user_details::user_uuid.eq(user_uuid))).set(schema::user_details::archived.eq(true)).execute(&self.connection))
    }
    pub fn delete_user_account(&mut self, user_uuid: Uuid) -> QueryResult<()> {
        DbConnection::check_query_processed(diesel::delete(schema::user_details::table.filter(schema::user_details::user_uuid.eq(user_uuid))).execute(&self.connection))
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

#[cfg(test)]
mod user_accounts_test {
    use crate::models::User;
    use crate::{DbConnection, new_secure_uuid_v4};
    use chrono::Utc;
    use std::str::FromStr;

    const USER_UUID: &str = "e78836e9-1982-4380-a678-a5b4db33d205";

    #[test]
    fn create_user_account() {
        let today = Utc::today();
        let user_id = uuid::Uuid::from_str(&USER_UUID).unwrap();
        let user = User {
            user_uuid: user_id,
            username: "Test_01".to_string(),
            password: "password".to_string(),
            email: "test@example.com".to_string(),
            date_of_birth: Utc::today().naive_utc(),
            join_date: Utc::today().naive_utc(),
            archived: false,
        };
        let mut con = DbConnection::new_connection();
        assert!(con.new_user_account(user).is_ok());
    }
    #[test]
    fn get_user_account() {
        let mut con = DbConnection::new_connection();
        let user_id = uuid::Uuid::from_str(&USER_UUID).unwrap();
        assert!(con.get_user_account(user_id).is_ok());
    }
    #[test]
    fn archive_user_account() {
        let mut con = DbConnection::new_connection();
        let user_id = uuid::Uuid::from_str(&USER_UUID).unwrap();
        assert!(con.archive_user_account(user_id).is_ok());
    }
    #[test]
    fn delete_user_account() {
        let mut con = DbConnection::new_connection();
        let user_id = uuid::Uuid::from_str(&USER_UUID).unwrap();
        assert!(con.delete_user_account(user_id).is_ok());
    }
}