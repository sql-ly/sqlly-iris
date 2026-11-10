use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Error, Result};

pub(crate) const HEADER_LEN: usize = 14;
const MAX_FRAME_LEN: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operation {
    Handshake,
    Connect,
    Disconnect,
    DirectQuery,
    Prepare,
    PreparedQuery,
    PreparedUpdate,
    CloseStatement,
    FetchData,
    GetServerError,
    Commit,
    Rollback,
    ReadStream,
    DirectStoredProcedure,
    MoreStoredProcedureResults,
    MoreResults,
    MultipleResultSetsFetchData,
    AutoCommitOff,
    AutoCommitOn,
    ReadCommitted,
    ReadUncommitted,
    ToggleSynchronousCommit,
    GlobalGet,
    GlobalSet,
    GlobalKill,
    GlobalOrder,
    GlobalData,
    ClassMethodValue,
    ClassMethodVoid,
    MethodValue,
    MethodVoid,
    PropertyGet,
    PropertySet,
    StreamSize,
    StreamSetBytes,
    StreamTruncate,
    CloseStream,
}

impl Operation {
    fn code(self) -> [u8; 2] {
        match self {
            Self::Handshake => *b"HS",
            Self::Connect => *b"CN",
            Self::Disconnect => *b"DC",
            Self::DirectQuery => *b"DQ",
            Self::Prepare => *b"PP",
            Self::PreparedQuery => *b"PQ",
            Self::PreparedUpdate => *b"PU",
            Self::CloseStatement => *b"CU",
            Self::FetchData => *b"FD",
            Self::GetServerError => *b"OE",
            Self::Commit => *b"TC",
            Self::Rollback => *b"TR",
            Self::ReadStream => *b"JS",
            Self::DirectStoredProcedure => *b"DS",
            Self::MoreStoredProcedureResults => *b"MS",
            Self::MoreResults => *b"MR",
            Self::MultipleResultSetsFetchData => *b"MD",
            Self::AutoCommitOff => *b"AF",
            Self::AutoCommitOn => *b"AN",
            Self::ReadCommitted => *b"RC",
            Self::ReadUncommitted => *b"RU",
            Self::ToggleSynchronousCommit => *b"TS",
            Self::GlobalGet => [0x41, 0xc2],
            Self::GlobalSet => [0x42, 0xc2],
            Self::GlobalKill => [0x43, 0xc2],
            Self::GlobalOrder => [0x45, 0xc2],
            Self::GlobalData => [0x49, 0xc2],
            Self::ClassMethodValue => [0x4b, 0xc2],
            Self::ClassMethodVoid => [0x4c, 0xc2],
            Self::MethodValue => [0x5b, 0xc2],
            Self::MethodVoid => [0x5c, 0xc2],
            Self::PropertyGet => [0x5d, 0xc2],
            Self::PropertySet => [0x5e, 0xc2],
            Self::StreamSize => *b"SS",
            Self::StreamSetBytes => *b"SZ",
            Self::StreamTruncate => *b"SX",
            Self::CloseStream => *b"CS",
        }
    }
}

#[derive(Debug)]
pub(crate) struct Frame {
    pub(crate) count: u32,
    pub(crate) statement_id: u32,
    pub(crate) status: u16,
    pub(crate) payload: Vec<u8>,
}

pub(crate) async fn write_frame<S>(
    stream: &mut S,
    operation: Operation,
    count: u32,
    statement_id: u32,
    payload: &[u8],
) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    let payload_len = u32::try_from(payload.len())
        .map_err(|_| Error::Protocol("outbound frame exceeds u32::MAX".into()))?;
    let mut header = [0_u8; HEADER_LEN];
    header[0..4].copy_from_slice(&payload_len.to_le_bytes());
    header[4..8].copy_from_slice(&count.to_le_bytes());
    header[8..12].copy_from_slice(&statement_id.to_le_bytes());
    header[12..14].copy_from_slice(&operation.code());
    stream.write_all(&header).await?;
    stream.write_all(payload).await?;
    stream.flush().await?;
    Ok(())
}

pub(crate) async fn read_frame<S>(stream: &mut S) -> Result<Frame>
where
    S: AsyncRead + Unpin,
{
    let mut header = [0_u8; HEADER_LEN];
    stream.read_exact(&mut header).await?;
    let payload_len = u32::from_le_bytes(header[0..4].try_into().unwrap()) as usize;
    if payload_len > MAX_FRAME_LEN {
        return Err(Error::Protocol(format!(
            "inbound frame length {payload_len} exceeds the {MAX_FRAME_LEN}-byte limit"
        )));
    }
    let count = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let statement_id = u32::from_le_bytes(header[8..12].try_into().unwrap());
    let status = u16::from_le_bytes(header[12..14].try_into().unwrap());
    let mut payload = vec![0; payload_len];
    stream.read_exact(&mut payload).await?;
    Ok(Frame {
        count,
        statement_id,
        status,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn frame_round_trip_preserves_header_and_payload() {
        let (mut left, mut right) = tokio::io::duplex(1024);
        let sender = tokio::spawn(async move {
            write_frame(&mut left, Operation::DirectQuery, 7, 9, b"abc")
                .await
                .unwrap();
        });
        let frame = read_frame(&mut right).await.unwrap();
        sender.await.unwrap();
        assert_eq!(frame.count, 7);
        assert_eq!(frame.statement_id, 9);
        assert_eq!(frame.status, u16::from_le_bytes(*b"DQ"));
        assert_eq!(frame.payload, b"abc");
    }

    #[tokio::test]
    async fn oversized_frames_are_rejected_before_allocation() {
        let (mut left, mut right) = tokio::io::duplex(1024);
        let sender = tokio::spawn(async move {
            let mut header = [0_u8; HEADER_LEN];
            header[0..4].copy_from_slice(&(MAX_FRAME_LEN as u32 + 1).to_le_bytes());
            left.write_all(&header).await.unwrap();
        });
        let error = read_frame(&mut right).await.unwrap_err();
        sender.await.unwrap();
        assert!(error.to_string().contains("exceeds"));
    }
}
