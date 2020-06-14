use super::schema::{bank_accounts, user_details};
extern crate chrono;
#[derive(Queryable, Insertable)]
#[table_name = "user_details"]
pub struct User {
    pub user_uuid: uuid::Uuid,
    pub username: String,
    pub password: String,
    pub email: String,
    pub date_of_birth: chrono::naive::NaiveDate,
    pub join_date: chrono::naive::NaiveDate,
    pub archived:bool
}

#[derive(Queryable, Insertable)]
#[table_name = "bank_accounts"]
pub struct Account {
    account_number: i32,
    user_uuid: uuid::Uuid,
    balance: bigdecimal::BigDecimal,
    sort_code: i32,
    interest_rate: bigdecimal::BigDecimal,
    overdraft_limit: bigdecimal::BigDecimal,
    account_name: Option<String>,
    account_category: Option<String>,
    archived:bool
}
