#![forbid(unsafe_code)]

use sqlly_iris::{Client, Config, IsolationLevel, Value};

#[tokio::test]
#[ignore = "requires IRIS_DSN and a disposable IRIS instance"]
async fn native_session_controls_cover_isolation_commit_and_rollback() {
    let dsn = std::env::var("IRIS_DSN").expect("IRIS_DSN must name a disposable IRIS instance");
    let mut client = Client::connect(Config::from_url(&dsn).unwrap())
        .await
        .unwrap();
    let table = format!("SqllyIrisSession{}", std::process::id());
    client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await
        .unwrap();
    client
        .execute(&format!("CREATE TABLE {table} (ID INTEGER)"), &[])
        .await
        .unwrap();

    client
        .set_isolation_level(IsolationLevel::ReadCommitted)
        .await
        .unwrap();
    assert_eq!(client.isolation_level(), IsolationLevel::ReadCommitted);
    match client.toggle_synchronous_commit().await {
        Ok(()) => client.toggle_synchronous_commit().await.unwrap(),
        Err(sqlly_iris::Error::Server { code: 409, .. }) => {}
        Err(error) => panic!("unexpected synchronous-commit error: {error}"),
    }

    client.set_autocommit(false).await.unwrap();
    assert!(!client.autocommit());
    client
        .execute(
            &format!("INSERT INTO {table} (ID) VALUES (?)"),
            &[1_i32.into()],
        )
        .await
        .unwrap();
    client.rollback().await.unwrap();
    assert!(
        client
            .query(&format!("SELECT ID FROM {table}"), &[])
            .await
            .unwrap()
            .collect()
            .await
            .unwrap()
            .is_empty()
    );

    client
        .execute(
            &format!("INSERT INTO {table} (ID) VALUES (?)"),
            &[2_i32.into()],
        )
        .await
        .unwrap();
    client.commit().await.unwrap();
    let rows = client
        .query(&format!("SELECT ID FROM {table}"), &[])
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get(0).unwrap(), &Value::I32(2));

    client
        .set_isolation_level(IsolationLevel::ReadUncommitted)
        .await
        .unwrap();
    assert_eq!(client.isolation_level(), IsolationLevel::ReadUncommitted);
    client.set_autocommit(true).await.unwrap();
    assert!(client.autocommit());
    client
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}
