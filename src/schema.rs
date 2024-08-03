// @generated automatically by Diesel CLI.

diesel::table! {
    book (id) {
        id -> Uuid,
        owner -> Uuid,
        #[max_length = 17]
        isbn -> Varchar,
        title -> Text,
        summary -> Text,
        published -> Nullable<Date>,
        publisher -> Nullable<Text>,
        language -> Nullable<Text>,
        googleid -> Nullable<Text>,
        goodreadsid -> Nullable<Text>,
        amazonid -> Nullable<Text>,
        librarythingid -> Nullable<Text>,
        pagecount -> Nullable<Int4>,
    }
}

diesel::table! {
    bookauthor (book, name) {
        book -> Uuid,
        name -> Text,
    }
}

diesel::table! {
    booktag (book, tag) {
        book -> Uuid,
        tag -> Text,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        name -> Text,
    }
}

diesel::joinable!(book -> users (owner));
diesel::joinable!(bookauthor -> book (book));
diesel::joinable!(booktag -> book (book));

diesel::allow_tables_to_appear_in_same_query!(
    book,
    bookauthor,
    booktag,
    users,
);
