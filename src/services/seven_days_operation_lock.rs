use anyhow::{Context as _, Result};
use sqlx::{Postgres, Transaction};
use std::time::Duration;

const LOCK_KEY: i64 = 7_071_007;
const LOCK_ACQUIRE_TIMEOUT: Duration = Duration::from_millis(500);

/// 7DTDの開始・停止をCloud Runの複数インスタンス間で直列化するトランザクションロック。
/// guardがdropされると未commit transactionがrollbackされ、ロックも解放される。
pub struct SevenDaysOperationLock {
    transaction: Option<Transaction<'static, Postgres>>,
}

impl SevenDaysOperationLock {
    pub async fn try_acquire(pool: &sqlx::PgPool) -> Result<Option<Self>> {
        let transaction = match tokio::time::timeout(LOCK_ACQUIRE_TIMEOUT, pool.begin()).await {
            Ok(transaction) => transaction.context("7DTD operation lock transaction failed")?,
            Err(_) => return Ok(None),
        };
        let mut transaction = transaction;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
            .bind(LOCK_KEY)
            .fetch_one(&mut *transaction)
            .await
            .context("7DTD operation lock query failed")?;
        if !acquired {
            return Ok(None);
        }
        Ok(Some(Self {
            transaction: Some(transaction),
        }))
    }
}

impl Drop for SevenDaysOperationLock {
    fn drop(&mut self) {
        let _ = self.transaction.take();
    }
}
