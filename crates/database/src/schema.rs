// @generated automatically by Diesel CLI.

diesel::table! {
    users (id) {
        id -> Integer,
        username -> Text,
        email -> Text,
    }
}

diesel::table! {
    content (id) {
        id -> Integer,
        title -> Text,
        description -> Text,
        author_id -> Integer,
    }
}
