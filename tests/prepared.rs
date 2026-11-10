#![forbid(unsafe_code)]

use futures_util::TryStreamExt;
use sqlly_iris::{Client, Config, Error, Value};

#[tokio::test]
#[ignore = "requires IRIS_DSN and a disposable IRIS instance"]
async fn prepared_statements_reuse_metadata_and_stream_rows() {
    let dsn = std::env::var("IRIS_DSN").expect("IRIS_DSN must name a disposable IRIS instance");
    let mut client = Client::connect(Config::from_url(&dsn).unwrap())
        .await
        .unwrap();
    let table = format!("SqllyIrisPrepared{}", std::process::id());
    client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await
        .unwrap();
    client
        .execute(
            &format!("CREATE TABLE {table} (ID INTEGER PRIMARY KEY, Name VARCHAR(100), Amount DECIMAL(19,4))"),
            &[],
        )
        .await
        .unwrap();

    let mut insert = client
        .prepare(&format!(
            "INSERT INTO {table} (ID, Name, Amount) VALUES (?, ?, ?)"
        ))
        .await
        .unwrap();
    assert_eq!(insert.parameter_count(), 3);
    for id in 1..=3_i32 {
        let result = insert
            .execute(&[
                id.into(),
                Value::String(format!("row {id}")),
                Value::Decimal(format!("{id}.25")),
            ])
            .await
            .unwrap();
        assert_eq!(result.rows_affected, 1);
    }
    assert!(matches!(
        insert.execute(&[4_i32.into()]).await,
        Err(Error::ParameterCount {
            expected: 3,
            actual: 1
        })
    ));
    insert.close().await.unwrap();

    let mut select = client
        .prepare(&format!(
            "SELECT ID, Name, Amount FROM {table} WHERE ID >= ? ORDER BY ID"
        ))
        .await
        .unwrap();
    let first = select
        .query(&[1_i32.into()])
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(first.len(), 3);
    assert_eq!(first[0].get(2).unwrap(), &Value::Decimal("1.25".into()));
    let remaining: Vec<_> = select
        .query(&[2_i32.into()])
        .await
        .unwrap()
        .into_stream()
        .try_collect()
        .await
        .unwrap();
    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0].get(0).unwrap(), &Value::I32(2));
    drop(select);

    client
        .query("SELECT 1", &[])
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    client
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}
