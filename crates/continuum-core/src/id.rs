/// Creates a time-ordered UUIDv7 identifier.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}
