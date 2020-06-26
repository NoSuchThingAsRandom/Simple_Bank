table! {
    bank_accounts (account_number) {
        account_number -> Int4,
        user_uuid -> Uuid,
        balance -> Numeric,
        sort_code -> Int4,
        interest_rate -> Numeric,
        overdraft_limit -> Numeric,
        account_name -> Nullable<Text>,
        account_category -> Nullable<Text>,
        archived -> Bool,
    }
}

table! {
    tokens (token) {
        token -> Text,
        client_uuid -> Uuid,
        start_date -> Timestamp,
        expiry_date -> Timestamp,
    }
}

table! {
    transactions (transaction_id) {
        transaction_id -> Uuid,
        user_responsible -> Uuid,
        time_issued -> Timestamp,
        amount -> Numeric,
        source_sort_code -> Int4,
        source_account_number -> Int4,
        dest_sort_code -> Int4,
        dest_account_number -> Int4,
    }
}

table! {
    user_details (user_uuid) {
        user_uuid -> Uuid,
        username -> Text,
        password -> Text,
        email -> Text,
        date_of_birth -> Date,
        join_date -> Date,
        archived -> Bool,
    }
}

allow_tables_to_appear_in_same_query!(
    bank_accounts,
    tokens,
    transactions,
    user_details,
);
