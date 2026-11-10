#![forbid(unsafe_code)]

use futures_util::TryStreamExt;
use sqlly_iris::{Client, Config, Value};

#[tokio::test]
#[ignore = "requires IRIS_DSN and a disposable IRIS instance"]
async fn procedures_return_multiple_large_results_and_clean_up() {
    let dsn = std::env::var("IRIS_DSN").expect("IRIS_DSN must name a disposable IRIS instance");
    let mut client = Client::connect(Config::from_url(&dsn).unwrap())
        .await
        .unwrap();
    let table = format!("SqllyIrisProcedureData{}", std::process::id());
    let procedure = format!("SqllyIrisProcedure{}", std::process::id());
    client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await
        .unwrap();
    client
        .execute(
            &format!("CREATE TABLE {table} (ID INTEGER, Name VARCHAR(2000))"),
            &[],
        )
        .await
        .unwrap();
    let mut insert = client
        .prepare(&format!("INSERT INTO {table} (ID, Name) VALUES (?, ?)"))
        .await
        .unwrap();
    for id in 1..=100_i32 {
        insert
            .execute(&[
                id.into(),
                Value::String(format!("row {id}: {}", "x".repeat(1000))),
            ])
            .await
            .unwrap();
    }
    insert.close().await.unwrap();
    client
        .execute(
            &format!(
                "CREATE OR REPLACE PROCEDURE {procedure}(IN MaxID INTEGER) DYNAMIC RESULT SETS 2 LANGUAGE OBJECTSCRIPT {{ SET tStatement=##class(%SQL.Statement).%New() DO tStatement.%Prepare(\"SELECT ID, Name FROM {table} WHERE ID <= ? ORDER BY ID\") SET a=tStatement.%Execute(MaxID) DO %sqlcontext.AddResultSet(a) DO tStatement.%Prepare(\"SELECT ID FROM {table} WHERE ID <= ? ORDER BY ID\") SET b=tStatement.%Execute(MaxID+1) DO %sqlcontext.AddResultSet(b) QUIT 1 }}"
            ),
            &[],
        )
        .await
        .unwrap();

    let results = client
        .call_procedure(&procedure, &[2_i32.into()])
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].return_value, Some(Value::String("\u{1}".into())));
    assert_eq!(results[0].rows.len(), 2);
    assert_eq!(results[0].rows[0].get(0).unwrap(), &Value::I32(1));
    assert_eq!(results[0].rows[1].get(0).unwrap(), &Value::I32(2));
    assert_eq!(results[1].rows.len(), 3);
    assert_eq!(results[1].rows[2].get(0).unwrap(), &Value::I32(3));

    let results: Vec<_> = client
        .call_procedure(&procedure, &[100_i32.into()])
        .await
        .unwrap()
        .into_stream()
        .try_collect()
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].rows.len(), 100);
    assert_eq!(results[1].rows.len(), 100);
    assert_eq!(results[0].rows[99].get(0).unwrap(), &Value::I32(100));
    assert_eq!(results[1].rows[99].get(0).unwrap(), &Value::I32(100));

    let scalar = format!("SqllyIrisScalar{}", std::process::id());
    client
        .execute(
            &format!("CREATE OR REPLACE PROCEDURE {scalar}() RETURNS INTEGER LANGUAGE OBJECTSCRIPT {{ QUIT 42 }}"),
            &[],
        )
        .await
        .unwrap();
    let mut call = client.call_procedure(&scalar, &[]).await.unwrap();
    let result = call.next_result().await.unwrap().unwrap();
    assert_eq!(result.return_value, Some(Value::I64(42)));
    assert!(result.rows.is_empty());
    assert!(call.next_result().await.unwrap().is_none());
    drop(call);
    client
        .execute(&format!("DROP PROCEDURE {scalar}"), &[])
        .await
        .unwrap();

    let mut call = client
        .call_procedure(&procedure, &[1_i32.into()])
        .await
        .unwrap();
    let _ = call.next_result().await.unwrap();
    drop(call);
    client
        .query("SELECT 1", &[])
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();

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
