use chrono::NaiveDate;
use diesel::{
    backend::Backend, expression::AsExpression, prelude::*, serialize::ToSql, sql_types::Text,
};
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

#[derive(Queryable, Selectable, Identifiable, PartialEq, Debug)]
#[diesel(table_name = crate::schema::author)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Author {
    pub id: i32,
    pub name: String,
}

#[derive(Insertable, AsExpression, Debug)]
#[diesel(table_name = crate::schema::author)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(sql_type = Text)]
pub struct AuthorName {
    pub name: String,
}

impl<DB> ToSql<Text, DB> for AuthorName
where
    DB: Backend,
    String: ToSql<Text, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        self.name.to_sql(out)
    }
}

#[derive(Insertable, Identifiable, Selectable, Queryable, Associations, Debug)]
#[diesel(belongs_to(BookPreview, foreign_key = book))]
#[diesel(belongs_to(BookComplete, foreign_key = book))]
#[diesel(belongs_to(Author, foreign_key = author))]
#[diesel(table_name = crate::schema::bookauthor)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(book, author))]
pub struct BookAuthor {
    pub book: Uuid,
    pub author: i32,
}

#[derive(Insertable, AsExpression, Debug)]
#[diesel(table_name = crate::schema::tag)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(sql_type = Text)]
pub struct TagName {
    pub name: String,
}

impl<DB> ToSql<Text, DB> for TagName
where
    DB: Backend,
    String: ToSql<Text, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        self.name.to_sql(out)
    }
}

#[derive(Insertable, Associations, Queryable, Selectable, Identifiable)]
#[diesel(table_name = crate::schema::booktag)]
#[diesel(belongs_to(BookComplete, foreign_key = book))]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(book, tag))]
pub struct BookTag {
    pub book: Uuid,
    pub tag: i32,
}

#[derive(Queryable, Selectable, Identifiable, Debug)]
#[diesel(table_name = crate::schema::book)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct BookPreview {
    pub id: Uuid,
    pub owner: Uuid,
    pub isbn: String,
    pub title: String,
    pub published: Option<NaiveDate>,
    pub owned: bool,
    pub read: bool,
}

#[derive(Selectable, Queryable, Identifiable)]
#[diesel(table_name = crate::schema::book)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct BookComplete {
    pub id: Uuid,
    pub owner: Uuid,
    pub isbn: String,
    pub title: String,
    pub summary: String,
    pub published: Option<NaiveDate>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub googleid: Option<String>,
    pub amazonid: Option<String>,
    pub librarythingid: Option<String>,
    pub pagecount: Option<i32>,
    pub owned: bool,
    pub read: bool,
}

#[derive(Insertable, Selectable, Queryable, Debug, AsChangeset)]
#[diesel(table_name = crate::schema::book)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(treat_none_as_null = true)]
pub struct Book {
    pub owner: Uuid,
    pub isbn: String,
    pub title: String,
    pub summary: String,
    pub published: Option<NaiveDate>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub googleid: Option<String>,
    pub amazonid: Option<String>,
    pub librarythingid: Option<String>,
    pub pagecount: Option<i32>,
    pub owned: bool,
    pub read: bool,
}

#[derive(Queryable, Identifiable, Selectable, Debug)]
#[diesel(table_name = crate::schema::book)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct BookId {
    pub id: Uuid,
}

#[derive(Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::series)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Series {
    pub owner: Uuid,
    pub name: String,
}

#[derive(Insertable, AsChangeset, Associations, Identifiable, Selectable, Queryable, Debug)]
#[diesel(table_name = crate::schema::bookseries)]
#[diesel(belongs_to(BookPreview, foreign_key = book))]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(book))]
pub struct BookSeries {
    pub book: Uuid,
    pub series: Uuid,
    pub number: i32,
}

#[derive(Insertable, Identifiable, Selectable, Queryable, Debug, QueryableByName)]
#[diesel(table_name = crate::schema::series)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SeriesInfo {
    pub id: Uuid,
    pub name: String,
}
