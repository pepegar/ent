use sqlx::decode::Decode;
use sqlx::encode::{Encode, IsNull};
use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef};
use sqlx::types::Type;
use std::ops::Deref;

// Value wrapper for xid8
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xid8(u64);

impl Xid8 {
    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn max() -> Self {
        Xid8(9223372036854775807)
    }
}

impl Deref for Xid8 {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Type<sqlx::Postgres> for Xid8 {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("xid8")
    }
}

impl<'r> Decode<'r, sqlx::Postgres> for Xid8 {
    fn decode(
        value: PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        let value = <i64 as Decode<sqlx::Postgres>>::decode(value)?;
        Ok(Xid8(value as u64))
    }
}

impl Encode<'_, sqlx::Postgres> for Xid8 {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        Encode::<sqlx::Postgres>::encode_by_ref(&(self.0 as i64), buf)
    }
}
