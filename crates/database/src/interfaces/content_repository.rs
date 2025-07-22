use diesel::prelude::*;

#[derive(Queryable, Selectable, AsChangeset, Debug)]
#[diesel(table_name = crate::schema::content)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Content {
    pub id: i32,
    pub title: String,
    pub description: String,
}

pub trait ContentRepository {
    /// Retrieves a content item by its ID.
    fn get(&self, id: i32) -> Result<Content, diesel::result::Error>;

    /// Creates a new content item.
    fn create(&self, content: Content) -> Result<Content, diesel::result::Error>;

    /// Updates an existing content item.
    fn update(&self, id: i32, content: Content) -> Result<Content, diesel::result::Error>;

    /// Deletes a content item.
    fn delete(&self, id: i32) -> Result<(), diesel::result::Error>;
}
