// @generated automatically by Diesel CLI.

diesel::table! {
    author (id) {
        id -> Int4,
        name -> Text,
    }
}

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
    bookauthor (book, author) {
        book -> Uuid,
        author -> Int4,
    }
}

diesel::table! {
    booktag (book, tag) {
        book -> Uuid,
        tag -> Int4,
    }
}

diesel::table! {
    tag (id) {
        id -> Int4,
        name -> Text,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        name -> Text,
    }
}

diesel::joinable!(book -> users (owner));
diesel::joinable!(bookauthor -> author (author));
diesel::joinable!(bookauthor -> book (book));
diesel::joinable!(booktag -> book (book));
diesel::joinable!(booktag -> tag (tag));

diesel::allow_tables_to_appear_in_same_query!(
    author,
    book,
    bookauthor,
    booktag,
    tag,
    users,
);
