#![forbid(unsafe_code)]

use sqlly_iris::{Client, Config, Value};

#[tokio::test]
#[ignore = "requires IRIS_DSN and a disposable IRIS instance"]
async fn native_stream_handles_read_write_truncate_and_close() {
    let dsn = std::env::var("IRIS_DSN").expect("IRIS_DSN must name a disposable IRIS instance");
    let mut client = Client::connect(Config::from_url(&dsn).unwrap())
        .await
        .unwrap();
    let table = format!("SqllyIrisStream{}", std::process::id());
    client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await
        .unwrap();
    client
        .execute(&format!("CREATE TABLE {table} (ID INTEGER PRIMARY KEY, Note LONGVARCHAR, Payload LONGVARBINARY)"), &[])
        .await
        .unwrap();
    client
        .execute(
            &format!("INSERT INTO {table} (ID, Note, Payload) VALUES (?, ?, ?)"),
            &[
                1_i32.into(),
                Value::from("hello world"),
                Value::Bytes(vec![0, 1, 2, 127, 128, 255]),
            ],
        )
        .await
        .unwrap();

    let row = client
        .query_with_handles(
            &format!("SELECT Note, Payload FROM {table} WHERE ID = 1"),
            &[],
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap()
        .remove(0);
    let (note, note_binary) = stream_handle(row.get(0).unwrap());
    let (payload, payload_binary) = stream_handle(row.get(1).unwrap());
    assert!(!note_binary);
    assert!(payload_binary);
    assert_eq!(client.stream_size(note).await.unwrap(), 11);
    assert_eq!(client.stream_size(payload).await.unwrap(), 6);
    assert_eq!(
        client.read_stream(note, 5, false).await.unwrap(),
        Value::from("hello")
    );
    assert_eq!(
        client.read_stream(note, 6, false).await.unwrap(),
        Value::from("hello ")
    );
    assert_eq!(
        client.read_stream(note, -1, false).await.unwrap(),
        Value::from("hello world")
    );
    assert_eq!(
        client.read_stream(payload, -1, true).await.unwrap(),
        Value::Bytes(vec![0, 1, 2, 127, 128, 255])
    );
    client.close_stream(payload).await.unwrap();

    let row = client
        .query_with_handles(&format!("SELECT Note FROM {table} WHERE ID = 1"), &[])
        .await
        .unwrap()
        .collect()
        .await
        .unwrap()
        .remove(0);
    let (note, _) = stream_handle(row.get(0).unwrap());
    client.stream_truncate(note, 5).await.unwrap();
    client.stream_set_bytes(note, 1, b"J").await.unwrap();
    client.close_stream(note).await.unwrap();
    let value = client
        .query(&format!("SELECT Note FROM {table} WHERE ID = 1"), &[])
        .await
        .unwrap()
        .collect()
        .await
        .unwrap()
        .remove(0)
        .into_values()
        .remove(0);
    assert_eq!(value, Value::from("Jello"));

    client
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

fn stream_handle(value: &Value) -> (&str, bool) {
    let Value::StreamHandle { reference, binary } = value else {
        panic!("expected a native stream handle, got {value:?}");
    };
    (reference, *binary)
}
