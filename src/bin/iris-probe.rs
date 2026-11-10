use sqlly_iris::{Client, Config, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut dsn = std::env::var("IRIS_DSN").ok();
    let mut sql = None;
    let mut execute = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--dsn" => dsn = arguments.next(),
            "--sql" => sql = arguments.next(),
            "--exec" => execute = true,
            "--help" | "-h" => {
                eprintln!(
                    "usage: iris-probe [--dsn URL] (--sql SQL) [--exec]\nThe IRIS_DSN environment variable is used when --dsn is omitted."
                );
                return Ok(());
            }
            _ => {
                eprintln!("unknown argument: {argument}");
                std::process::exit(2);
            }
        }
    }
    let dsn = dsn
        .ok_or_else(|| sqlly_iris::Error::InvalidConfig("provide --dsn or set IRIS_DSN".into()))?;
    let sql = sql.ok_or_else(|| {
        sqlly_iris::Error::InvalidConfig("provide --sql with a statement to run".into())
    })?;
    let config = Config::from_url(&dsn)?;
    let mut client = Client::connect(config).await?;
    println!("connected: {:?}", client.server_info());
    if execute {
        let result = client.execute(&sql, &[]).await?;
        println!("rows affected: {}", result.rows_affected);
    } else {
        let mut stream = client.query(&sql, &[]).await?;
        println!("columns: {:#?}", stream.columns());
        let mut count = 0_u64;
        while let Some(row) = stream.next_row().await? {
            count += 1;
            println!("row {count}: {:#?}", row.values());
        }
        println!("rows returned: {count}");
    }
    client.close().await?;
    Ok(())
}
