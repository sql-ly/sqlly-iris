#![forbid(unsafe_code)]

use sqlly_iris::{Client, Config};

#[tokio::test]
#[ignore = "requires IRIS_DSN and a disposable IRIS instance"]
async fn catalogs_list_schema_objects_and_procedure_metadata() {
    let dsn = std::env::var("IRIS_DSN").expect("IRIS_DSN must name a disposable IRIS instance");
    let mut client = Client::connect(Config::from_url(&dsn).unwrap())
        .await
        .unwrap();
    let table = format!("SqllyIrisCatalog{}", std::process::id());
    let procedure = format!("SqllyIrisCatalogProc{}", std::process::id());
    assert!(
        client
            .list_schemas()
            .await
            .unwrap()
            .iter()
            .any(|schema| schema.name == "SQLUser")
    );
    client
        .execute(
            &format!("CREATE TABLE {table} (ID INTEGER NOT NULL, Name VARCHAR(100))"),
            &[],
        )
        .await
        .unwrap();
    assert!(
        client
            .list_tables("SQLUser")
            .await
            .unwrap()
            .iter()
            .any(|item| item.name == table)
    );
    let columns = client.list_columns("SQLUser", &table).await.unwrap();
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0].name, "ID");
    assert!(!columns[0].nullable);
    assert_eq!(columns[0].ordinal_position, 1);
    assert_eq!(columns[1].name, "Name");
    assert_eq!(columns[1].character_maximum_length, Some(100));
    client
        .execute(
            &format!("CREATE OR REPLACE PROCEDURE {procedure}() RETURNS INTEGER LANGUAGE OBJECTSCRIPT {{ QUIT 1 }}"),
            &[],
        )
        .await
        .unwrap();
    assert!(
        client
            .list_procedures("SQLUser")
            .await
            .unwrap()
            .iter()
            .any(|item| item.name == procedure)
    );
    client
        .execute(&format!("DROP PROCEDURE {procedure}"), &[])
        .await
        .unwrap();
    client
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .unwrap();
    client.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires IRIS_DSN and a disposable IRIS instance"]
async fn tls_required_never_falls_back_to_plaintext() {
    let dsn = std::env::var("IRIS_DSN").expect("IRIS_DSN must name a disposable IRIS instance");
    let mut config = Config::from_url(&dsn).unwrap();
    config.encryption(sqlly_iris::Encryption::Required);
    let result = Client::connect(config).await;
    assert!(matches!(result, Err(sqlly_iris::Error::Tls(_))));
}
