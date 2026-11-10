#![forbid(unsafe_code)]

use sqlly_iris::{Client, Config, GlobalDirection, Value};

#[tokio::test]
#[ignore = "requires IRIS_DSN and a disposable IRIS instance"]
async fn globals_objects_methods_and_properties_round_trip() {
    let dsn = std::env::var("IRIS_DSN").expect("IRIS_DSN must name a disposable IRIS instance");
    let mut client = Client::connect(Config::from_url(&dsn).unwrap())
        .await
        .unwrap();
    let global = format!("SqllyIrisNative{}", std::process::id());
    client.global_kill(&global, &[]).await.unwrap();
    assert_eq!(client.global_get(&global, &[]).await.unwrap(), Value::Null);
    let data = client.global_data(&global, &[]).await.unwrap();
    assert!(!data.exists);
    assert!(!data.has_children);

    client
        .global_set(&global, &Value::from("alpha"), &["a".into()])
        .await
        .unwrap();
    client
        .global_set(&global, &Value::from("beta"), &["b".into()])
        .await
        .unwrap();
    assert_eq!(
        client.global_get(&global, &["a".into()]).await.unwrap(),
        Value::from("alpha")
    );
    let data = client.global_data(&global, &[]).await.unwrap();
    assert!(!data.exists);
    assert!(data.has_children);
    assert_eq!(
        client
            .global_order(&global, "", &[], GlobalDirection::Next)
            .await
            .unwrap(),
        Some("a".into())
    );
    assert_eq!(
        client
            .global_order(&global, "a", &[], GlobalDirection::Next)
            .await
            .unwrap(),
        Some("b".into())
    );
    assert_eq!(
        client
            .global_order(&global, "b", &[], GlobalDirection::Next)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        client
            .global_order(&global, "", &[], GlobalDirection::Previous)
            .await
            .unwrap(),
        Some("b".into())
    );
    client.global_kill(&global, &[]).await.unwrap();
    assert_eq!(
        client.global_get(&global, &["a".into()]).await.unwrap(),
        Value::Null
    );

    let version = client
        .class_method("%SYSTEM.Version", "GetVersion", &[])
        .await
        .unwrap();
    assert!(matches!(version, Value::String(value) if value.contains("IRIS")));
    let object = client
        .class_method("%SQL.Statement", "%New", &[])
        .await
        .unwrap();
    let Value::ObjectRef(object) = object else {
        panic!("expected an object reference");
    };
    client
        .property_set(&object, "%SelectMode", &Value::I32(2))
        .await
        .unwrap();
    assert_eq!(
        client.property_get(&object, "%SelectMode").await.unwrap(),
        Value::I64(2)
    );

    let stream = client
        .class_method("%Stream.GlobalCharacter", "%New", &[])
        .await
        .unwrap();
    let Value::ObjectRef(stream) = stream else {
        panic!("expected a stream object reference");
    };
    client
        .method(&stream, "Write", &["hello".into()])
        .await
        .unwrap();
    assert_eq!(
        client
            .method(&stream, "Read", &[5_i32.into()])
            .await
            .unwrap(),
        Value::from("hello")
    );
    client.close().await.unwrap();
}
