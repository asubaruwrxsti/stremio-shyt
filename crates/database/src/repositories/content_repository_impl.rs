use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};

use crate::interfaces::{Content, ContentRepository};
use crate::schema::content;

pub struct ContentRepositoryImpl {
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl ContentRepositoryImpl {
    pub fn new(pool: Pool<ConnectionManager<SqliteConnection>>) -> Self {
        Self { pool }
    }
}

impl ContentRepository for ContentRepositoryImpl {
    fn get(&self, id: i32) -> Result<Content, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get SQLite connection");
        content::table
            .filter(content::id.eq(id))
            .select(Content::as_select())
            .first::<Content>(&mut conn)
    }

    fn create(&self, content: Content) -> Result<Content, diesel::result::Error> {
        use crate::schema::content::dsl::*;

        let conn = self.pool.get()?;
        diesel::insert_into(content)
            .values(content)
            .get_result(&conn)
    }

    fn update(&self, id: i32, content: Content) -> Result<Content, diesel::result::Error> {
        use crate::schema::content::dsl::*;

        let conn = self.pool.get()?;
        diesel::update(content.find(id))
            .set(content)
            .get_result(&conn)
    }

    fn delete(&self, id: i32) -> Result<(), diesel::result::Error> {
        use crate::schema::content::dsl::*;

        let conn = self.pool.get()?;
        diesel::delete(content.find(id)).execute(&conn)?;
        Ok(())
    }
}
