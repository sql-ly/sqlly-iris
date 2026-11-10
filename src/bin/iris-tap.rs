use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

use tokio::{
    fs::OpenOptions,
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listen = None;
    let mut target = None;
    let mut output = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--listen" => listen = arguments.next(),
            "--target" => target = arguments.next(),
            "--output" => output = arguments.next().map(PathBuf::from),
            "--help" | "-h" => {
                eprintln!(
                    "usage: iris-tap --listen 127.0.0.1:21973 --target 127.0.0.1:21972 --output capture.tap\nCaptures contain session bytes and must be treated as sensitive."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    let listen = listen.ok_or("--listen is required")?;
    let target = target.ok_or("--target is required")?;
    let output = output.ok_or("--output is required")?;
    let listener = TcpListener::bind(&listen).await?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .await?;
    let file = Arc::new(Mutex::new(file.into_std().await));
    let started = Instant::now();
    eprintln!("listening on {listen}, forwarding to {target}");
    let mut connection_id = 0_u64;
    loop {
        let (client, peer) = listener.accept().await?;
        let target = target.clone();
        let file = Arc::clone(&file);
        let id = connection_id;
        connection_id += 1;
        tokio::spawn(async move {
            if let Err(error) = relay(client, &target, id, file, started).await {
                eprintln!("connection {id} from {peer}: {error}");
            }
        });
    }
}

async fn relay(
    client: TcpStream,
    target: &str,
    connection_id: u64,
    file: Arc<Mutex<std::fs::File>>,
    started: Instant,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = TcpStream::connect(target).await?;
    let (client_reader, client_writer) = tokio::io::split(client);
    let (server_reader, server_writer) = tokio::io::split(server);
    let upload = pump(
        client_reader,
        server_writer,
        connection_id,
        "c>s",
        Arc::clone(&file),
        started,
    );
    let download = pump(
        server_reader,
        client_writer,
        connection_id,
        "s>c",
        file,
        started,
    );
    tokio::try_join!(upload, download)?;
    Ok(())
}

async fn pump<R, W>(
    mut reader: R,
    mut writer: W,
    connection_id: u64,
    direction: &'static str,
    file: Arc<Mutex<std::fs::File>>,
    started: Instant,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    use std::io::Write;

    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            writer.shutdown().await?;
            return Ok(());
        }
        let elapsed = started.elapsed().as_micros();
        let record = format!(
            "{connection_id} {direction} {elapsed} {count} {}\n",
            hex(&buffer[..count])
        );
        {
            let mut file = file.lock().map_err(|_| "capture lock is poisoned")?;
            file.write_all(record.as_bytes())?;
            file.flush()?;
        }
        writer.write_all(&buffer[..count]).await?;
        writer.flush().await?;
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
