use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};

use crate::interfaces::{NewUser, User, UserRepository};
use crate::schema::users;

pub struct UserRepositoryImpl {
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl UserRepositoryImpl {
    pub fn new(pool: Pool<ConnectionManager<SqliteConnection>>) -> Self {
        Self { pool }
    }
}

impl UserRepository for UserRepositoryImpl {
    fn get_by_id(&self, user_id: i32) -> Result<User, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get SQLite connection");
        users::table
            .filter(users::id.eq(user_id))
            .select(User::as_select())
            .first::<User>(&mut conn)
    }

    fn create(&self, username: &str, email: &str) -> Result<User, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get SQLite connection");
        let new_user = NewUser { username, email };

        // SQLite doesn't support RETURNING, so we insert and then fetch
        diesel::insert_into(users::table)
            .values(&new_user)
            .execute(&mut conn)?;

        // Get the last inserted row
        users::table
            .order(users::id.desc())
            .select(User::as_select())
            .first::<User>(&mut conn)
    }

    fn update(&self, user: &User) -> Result<User, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get SQLite connection");

        diesel::update(users::table.filter(users::id.eq(user.id)))
            .set((
                users::username.eq(&user.username),
                users::email.eq(&user.email),
            ))
            .execute(&mut conn)?;

        // Fetch the updated user
        self.get_by_id(user.id)
    }

    fn delete(&self, user_id: i32) -> Result<(), diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get SQLite connection");
        diesel::delete(users::table.filter(users::id.eq(user_id))).execute(&mut conn)?;
        Ok(())
    }
}
