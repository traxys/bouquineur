use diesel::prelude::*;
use uuid::Uuid;

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::book)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Book {
    pub id: Uuid,
    pub isbn: String,
}
