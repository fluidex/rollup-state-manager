use fluidex_common::db::models::{account, operation_log, tablenames};
use fluidex_common::db::{DbType, TimestampDbType};
use serde::ser::Serializer;
use super::sqlxextend;

pub type DecimalDbType = fluidex_common::rust_decimal::Decimal;

/// Helper trait add serde support to `TimestampDbType` using milliseconds.
pub trait DateTimeMilliseconds: Sized {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
}

impl DateTimeMilliseconds for TimestampDbType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(self.timestamp_millis())
    }
}

/* --------------------- models::AccountDesc -----------------------------*/
impl sqlxextend::TableSchemas for account::AccountDesc {
    fn table_name() -> &'static str {
        tablenames::ACCOUNT
    }
    const ARGN: i32 = 3;
}

impl sqlxextend::BindQueryArg<'_, DbType> for account::AccountDesc {
    fn bind_args<'g, 'q: 'g>(&'q self, arg: &mut impl sqlx::Arguments<'g, Database = DbType>) {
        arg.add(self.id);
        arg.add(&self.l1_address);
        arg.add(&self.l2_pubkey);
    }
}

impl sqlxextend::SqlxAction<'_, sqlxextend::InsertTable, DbType> for account::AccountDesc {}

/* --------------------- models::OperationLog -----------------------------*/
impl sqlxextend::TableSchemas for operation_log::OperationLog {
    const ARGN: i32 = 4;
    fn table_name() -> &'static str {
        tablenames::OPERATION_LOG
    }
}

impl sqlxextend::BindQueryArg<'_, DbType> for operation_log::OperationLog {
    fn bind_args<'g, 'q: 'g>(&'q self, arg: &mut impl sqlx::Arguments<'g, Database = DbType>) {
        arg.add(self.id);
        arg.add(self.time);
        arg.add(&self.method);
        arg.add(&self.params);
    }
}

impl sqlxextend::SqlxAction<'_, sqlxextend::InsertTable, DbType> for operation_log::OperationLog {}
