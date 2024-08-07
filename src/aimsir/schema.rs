// @generated automatically by Diesel CLI.

diesel::table! {
    peer_tags (peer_id, tag_id) {
        peer_id -> Text,
        tag_id -> Integer,
    }
}

diesel::table! {
    peers (peer_id) {
        peer_id -> Text,
        name -> Text,
    }
}

diesel::table! {
    tags (id) {
        id -> Integer,
        parent -> Nullable<Integer>,
        name -> Text,
    }
}

diesel::joinable!(peer_tags -> peers (peer_id));
diesel::joinable!(peer_tags -> tags (tag_id));

diesel::allow_tables_to_appear_in_same_query!(
    peer_tags,
    peers,
    tags,
);
