#![forbid(unsafe_code)]

use sqlly_iris::{Client, Config, Value};

#[tokio::test]
#[ignore = "requires IRIS_DSN and a disposable IRIS instance"]
async fn parameter_arrays_execute_in_one_round_trip() {
    let dsn = std::env::var("IRIS_DSN").expect("IRIS_DSN must name a disposable IRIS instance");
    let mut config = Config::from_url(&dsn).unwrap();
    config.fast_insert(true);
    let mut client = Client::connect(config).await.unwrap();
    let table = format!("SqllyIrisBulk{}", std::process::id());
    client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await
        .unwrap();
    client
        .execute(
            &format!("CREATE TABLE {table} (ID IDENTITY, Name VARCHAR(100))"),
            &[],
        )
        .await
        .unwrap();
    let mut insert = client
        .prepare(&format!("INSERT INTO {table} (Name) VALUES (?)"))
        .await
        .unwrap();
    assert_eq!(insert.execute_many(&[]).await.unwrap().rows_affected, 0);
    let batches: Vec<Vec<Value>> = (1..=3)
        .map(|id| vec![Value::String(format!("row {id}"))])
        .collect();
    assert_eq!(
        insert.execute_many(&batches).await.unwrap().rows_affected,
        3
    );
    insert.close().await.unwrap();

    let rows = client
        .query(&format!("SELECT ID, Name FROM {table} ORDER BY ID"), &[])
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2].get(0).unwrap(), &Value::I64(3));
    client
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}
