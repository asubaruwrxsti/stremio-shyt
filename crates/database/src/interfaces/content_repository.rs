#[derive(Queryable, Selectable, Insertable)]
pub struct Content {
    pub id: i32,
    pub title: String,
    pub description: String,
}
