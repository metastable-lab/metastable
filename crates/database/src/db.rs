use mongodb::{Client, Database};

pub async fn get_db(uri: &str, db_name: &str) -> Database {
    let client = Client::with_uri_str(uri).await.unwrap();
    client.database(db_name)
}
