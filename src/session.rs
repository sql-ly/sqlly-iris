use crate::{Client, Result, protocol::Operation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    ReadUncommitted = 1,
    ReadCommitted = 2,
}

impl Client {
    pub fn autocommit(&self) -> bool {
        self.autocommit
    }

    pub async fn set_autocommit(&mut self, enabled: bool) -> Result<()> {
        let operation = if enabled {
            Operation::AutoCommitOn
        } else {
            Operation::AutoCommitOff
        };
        self.session_send(operation).await?;
        self.autocommit = enabled;
        Ok(())
    }

    pub async fn set_isolation_level(&mut self, level: IsolationLevel) -> Result<()> {
        let operation = match level {
            IsolationLevel::ReadCommitted => Operation::ReadCommitted,
            IsolationLevel::ReadUncommitted => Operation::ReadUncommitted,
        };
        self.session_send(operation).await?;
        self.isolation = level;
        self.server_info.isolation_level = level as i64;
        Ok(())
    }

    pub fn isolation_level(&self) -> IsolationLevel {
        self.isolation
    }

    pub async fn toggle_synchronous_commit(&mut self) -> Result<()> {
        self.session_frame(Operation::ToggleSynchronousCommit)
            .await
            .map(drop)
    }

    pub async fn commit(&mut self) -> Result<()> {
        self.session_frame(Operation::Commit).await.map(drop)
    }

    pub async fn rollback(&mut self) -> Result<()> {
        self.session_frame(Operation::Rollback).await.map(drop)
    }
}
