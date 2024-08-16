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
        owned -> Bool,
        read -> Bool,
    }
}

diesel::table! {
    bookauthor (book, author) {
        book -> Uuid,
        author -> Int4,
    }
}

diesel::table! {
    bookseries (book) {
        book -> Uuid,
        series -> Uuid,
        number -> Int4,
    }
}

diesel::table! {
    booktag (book, tag) {
        book -> Uuid,
        tag -> Int4,
    }
}

diesel::table! {
    series (id) {
        id -> Uuid,
        owner -> Uuid,
        name -> Text,
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

diesel::table! {
    wish (id) {
        id -> Uuid,
        owner -> Uuid,
        name -> Text,
    }
}

diesel::table! {
    wishauthor (wish, author) {
        wish -> Uuid,
        author -> Int4,
    }
}

diesel::table! {
    wishseries (wish) {
        wish -> Uuid,
        series -> Uuid,
        number -> Int4,
    }
}

diesel::joinable!(book -> users (owner));
diesel::joinable!(bookauthor -> author (author));
diesel::joinable!(bookauthor -> book (book));
diesel::joinable!(bookseries -> book (book));
diesel::joinable!(bookseries -> series (series));
diesel::joinable!(booktag -> book (book));
diesel::joinable!(booktag -> tag (tag));
diesel::joinable!(series -> users (owner));
diesel::joinable!(wish -> users (owner));
diesel::joinable!(wishauthor -> author (author));
diesel::joinable!(wishauthor -> wish (wish));
diesel::joinable!(wishseries -> series (series));
diesel::joinable!(wishseries -> wish (wish));

diesel::allow_tables_to_appear_in_same_query!(
    author,
    book,
    bookauthor,
    bookseries,
    booktag,
    series,
    tag,
    users,
    wish,
    wishauthor,
    wishseries,
);
