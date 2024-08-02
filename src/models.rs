use diesel::prelude::*;
use uuid::Uuid;

#[derive(Insertable)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewUser<'a> {
    pub name: &'a str,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub name: String,
    pub id: Uuid,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::book)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Book {
    pub id: Uuid,
    pub isbn: String,
}
