#![forbid(unsafe_code)]

use sqlly_iris::{Client, Config, Error, Value};

#[tokio::test]
#[ignore = "requires IRIS_DSN and a disposable IRIS instance"]
async fn sql_types_streams_pagination_and_transactions() {
    let dsn = std::env::var("IRIS_DSN").expect("IRIS_DSN must name a disposable IRIS instance");
    let mut config = Config::from_url(&dsn).unwrap();
    config.max_rows(0);
    let mut client = Client::connect(config).await.unwrap();
    assert!(client.server_info().product_info.contains("IRIS"));
    let table = format!("SqllyIrisProbe{}", std::process::id());
    client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await
        .unwrap();
    client
        .execute(
            &format!(
                "CREATE TABLE {table} (ID INTEGER PRIMARY KEY, Name VARCHAR(2000), Note LONGVARCHAR, Payload VARBINARY(100), Amount DECIMAL(38,9), Score DOUBLE, Enabled BIT, BigValue BIGINT, CreatedAt TIMESTAMP)"
            ),
            &[],
        )
        .await
        .unwrap();

    let insert = format!(
        "INSERT INTO {table} (ID, Name, Note, Payload, Amount, Score, Enabled, BigValue, CreatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
    );
    let result = client
        .execute(
            &insert,
            &[
                1_i32.into(),
                " café 東京 ".into(),
                "character stream contents".into(),
                vec![0, 1, 2, 127, 128, 255].into(),
                Value::Decimal("1234567890.123456789".into()),
                42.5_f64.into(),
                true.into(),
                i64::MAX.into(),
                Value::Timestamp("2026-11-09 13:14:15.123456".into()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);

    let row = client
        .query(
            &format!("SELECT ID, Name, Note, Payload, Amount, Score, Enabled, BigValue, CreatedAt FROM {table} WHERE ID = ?"),
            &[1_i32.into()],
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap()
        .remove(0);
    assert_eq!(row.get(0).unwrap(), &Value::I32(1));
    assert_eq!(row.get(1).unwrap(), &Value::from(" café 東京 "));
    assert_eq!(
        row.get(2).unwrap(),
        &Value::from("character stream contents")
    );
    assert_eq!(
        row.get(3).unwrap(),
        &Value::Bytes(vec![0, 1, 2, 127, 128, 255])
    );
    assert_eq!(
        row.get(4).unwrap(),
        &Value::Decimal("1234567890.123456789".into())
    );
    assert_eq!(row.get(5).unwrap(), &Value::F64(42.5));
    assert_eq!(row.get(6).unwrap(), &Value::Bool(true));
    assert_eq!(row.get(7).unwrap(), &Value::I64(i64::MAX));
    assert_eq!(
        row.get(8).unwrap(),
        &Value::PosixTimestamp {
            seconds: 1_794_230_055,
            micros: 123_456,
        }
    );

    client
        .execute(
            &format!("INSERT INTO {table} (ID, CreatedAt) VALUES (?, ?)"),
            &[
                2_i32.into(),
                Value::PosixTimestamp {
                    seconds: 1_794_230_055,
                    micros: 123_456,
                },
            ],
        )
        .await
        .unwrap();
    let timestamp_rows = client
        .query(
            &format!("SELECT CreatedAt FROM {table} WHERE ID = ?"),
            &[2_i32.into()],
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(timestamp_rows[0].get(0).unwrap(), row.get(8).unwrap());

    for id in 3..=100_i32 {
        client
            .execute(
                &format!("INSERT INTO {table} (ID, Name) VALUES (?, ?)"),
                &[
                    id.into(),
                    Value::String(format!("row {id}: {}", "x".repeat(1000))),
                ],
            )
            .await
            .unwrap();
    }
    let rows = client
        .query(&format!("SELECT ID, Name FROM {table} ORDER BY ID"), &[])
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(rows.len(), 100);

    let mut transaction = client.begin().await.unwrap();
    transaction
        .execute(
            &format!("INSERT INTO {table} (ID) VALUES (?)"),
            &[101_i32.into()],
        )
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    let mut transaction = client.begin().await.unwrap();
    transaction
        .execute(
            &format!("INSERT INTO {table} (ID) VALUES (?)"),
            &[102_i32.into()],
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    let rows = client
        .query(
            &format!("SELECT ID FROM {table} WHERE ID >= ? ORDER BY ID"),
            &[101_i32.into()],
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get(0).unwrap(), &Value::I32(102));

    {
        let error = client
            .query("SELECT * FROM Table_That_Does_Not_Exist", &[])
            .await;
        assert!(matches!(error, Err(Error::Server { .. })));
    }
    client
        .query("SELECT '?', /* ? */ 1", &[])
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();

    client
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
    let stream = client.query("SELECT 1", &[]).await.unwrap();
    drop(stream);
    assert!(matches!(
        client.query("SELECT 1", &[]).await,
        Err(Error::ConnectionUnusable(_))
    ));
    client.close().await.unwrap();
}
