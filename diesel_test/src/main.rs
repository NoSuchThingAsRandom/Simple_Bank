#[macro_use]
extern crate diesel;
extern crate dotenv;

pub mod db_connection;
pub mod models;
pub mod schema;
fn main() {
    let conn = db_connection::establish_connection();
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
