use diesel::prelude::*;

#[derive(Queryable, Selectable, AsChangeset, Debug)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: i32,
    pub username: String,
    pub email: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::users)]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub email: &'a str,
}

pub trait UserRepository {
    /// Fetch a user by ID
    fn get_by_id(&self, id: i32) -> Result<User, diesel::result::Error>;

    /// Create a new user
    fn create(&self, username: &str, email: &str) -> Result<User, diesel::result::Error>;

    /// Update a user
    fn update(&self, user: &User) -> Result<User, diesel::result::Error>;

    /// Delete a user
    fn delete(&self, id: i32) -> Result<(), diesel::result::Error>;
}
