use anyhow::Result;
use crate::storage::database::{DatabaseWriter, DatabaseWriterConfig};
use fluidex_common::db::DbType;
use fluidex_common::db::models::account;

type UserWriter = DatabaseWriter<account::AccountDesc>;

pub trait HistoryWriter: Sync + Send {
    fn is_block(&self) -> bool;
    fn append_user(&mut self, user: account::AccountDesc);
}

pub struct DummyHistoryWriter;
impl HistoryWriter for DummyHistoryWriter {
    fn is_block(&self) -> bool { false }
    fn append_user(&mut self, _user: account::AccountDesc) {}
}

pub struct DatabaseHistoryWriter {
    pub user_writer: UserWriter,
}

impl DatabaseHistoryWriter {
    pub fn new(config: &DatabaseWriterConfig, pool: &sqlx::Pool<DbType>) -> Result<DatabaseHistoryWriter> {
        Ok(DatabaseHistoryWriter {
            user_writer: UserWriter::new(config).start_schedule(pool)?,
        })
    }
}

impl HistoryWriter for DatabaseHistoryWriter {
    fn is_block(&self) -> bool { false }
    fn append_user(&mut self, user: account::AccountDesc) {
        self.user_writer.append(user).ok();
    }
}
