use super::schema::{bank_accounts, tokens, user_details};
use serde::{Deserialize, Serialize};
#[derive(Queryable, Insertable, Serialize, Deserialize)]
#[table_name = "user_details"]
pub struct User {
    pub user_uuid: uuid::Uuid,
    pub username: String,
    pub password: String,
    pub email: String,
    pub date_of_birth: chrono::naive::NaiveDate,
    pub join_date: chrono::naive::NaiveDate,
    pub archived: bool,
}

#[derive(Queryable, Insertable, Serialize, Deserialize)]
#[table_name = "bank_accounts"]
pub struct Account {
    pub account_number: i32,
    pub user_uuid: uuid::Uuid,
    pub balance: bigdecimal::BigDecimal,
    pub sort_code: i32,
    pub interest_rate: bigdecimal::BigDecimal,
    pub overdraft_limit: bigdecimal::BigDecimal,
    pub account_name: Option<String>,
    pub account_category: Option<String>,
    pub archived: bool,
}
/*impl Account {
    fn account_to_vec(&mut self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut data: Vec<String> = Vec::new();
        data.push(self.account_number?);
        data.push(self.user_uuid?);
        data.push(self.balance?);
        data.push(self.sort_code?);
        data.push(self.interest_rate?);
        data.push(self.account_number?);
        Ok(data)
    }
}*/
#[derive(Queryable, Insertable, Serialize, Deserialize)]
#[table_name = "tokens"]
pub struct Token {
    pub token: String,
    pub client_uuid: uuid::Uuid,
    pub start_date: chrono::NaiveDateTime,
    pub expiry_date: chrono::NaiveDateTime,
}
