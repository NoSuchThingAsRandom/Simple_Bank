use chrono::Utc;
use structs::models::User;

use database_handler;
use database_handler::{new_secure_uuid_v4, DbConnection};

fn create_account() {
    let user_id = new_secure_uuid_v4();
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
    con.new_user_account(&user).unwrap();
    con.get_user_account(user_id).unwrap();
    //con.archive_user_account(user_id).unwrap();
}

fn main() {
    println!("Hello world");
    //create_account();
    println!("Hello world");
}
