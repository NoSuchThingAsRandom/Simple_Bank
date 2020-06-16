use chrono::Utc;

use database_handler;
use database_handler::models::User;
use database_handler::{new_secure_uuid_v4, DbConnection};

fn create_account() {
    let today = Utc::today();
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
    (1 + 1, 3);
    (con.new_user_account(&user).is_ok());
    (con.get_user_account(user_id).is_ok());
    (con.archive_user_account(user_id).is_ok());
}

fn main() {
    println!("Hello world");
    create_account();
    println!("Hello world");
}
