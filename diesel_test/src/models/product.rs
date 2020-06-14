use crate::schema::products;
use crate::schema::products::dsl;
use diesel::PgConnection;
use diesel::QueryDsl;
use diesel::RunQueryDsl;

#[derive(Queryable)]
pub struct Product {
    pub id: i32,
    pub name: String,
    pub stock: f64,
    pub price: Option<i32>,
}

impl Product {
    pub fn find(id: &i32, connection: PgConnection) -> Result<Product, diesel::result::Error> {
        products::table.find(id).first(&connection)
    }
    pub fn destroy(id: &i32, connection: PgConnection) -> Result<(), diesel::result::Error> {
        diesel::delete(dsl::products.find(id)).execute(&connection)?;
        Ok(())
    }
}

#[derive(Insertable)]
#[table_name = "products"]
pub struct NewProduct {
    pub name: String,
    pub stock: f64,
    pub price: i32,
}
impl NewProduct {
    pub fn new(name: String, stock: f64, price: i32) -> NewProduct {
        NewProduct { name, stock, price }
    }

    pub fn create(&self, connection: PgConnection) -> Result<Product, diesel::result::Error> {
        diesel::insert_into(products::table)
            .values(self)
            .get_result(&connection)
    }
}
pub struct ProductList(pub Vec<Product>);

impl ProductList {
    pub fn list() -> Self {
        use crate::db_connection::establish_connection;
        use crate::schema::products::dsl::*;
        use diesel::QueryDsl;
        use diesel::RunQueryDsl;

        let connection = establish_connection();

        let result = products
            .limit(10)
            .load::<Product>(&connection)
            .expect("Error loading products");

        // We return a value by leaving it without a comma
        ProductList(result)
    }
}
