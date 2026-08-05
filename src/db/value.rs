//! Backend-agnostic SQL values and row decoding.

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqlValue {
    pub fn from_json(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Integer(i64::from(*b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Self::Integer(i)
                } else if let Some(u) = n.as_u64() {
                    Self::Integer(u as i64)
                } else {
                    Self::Real(n.as_f64().unwrap_or(0.0))
                }
            }
            serde_json::Value::String(s) => Self::Text(s.clone()),
            other => Self::Text(other.to_string()),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Integer(i) => serde_json::json!(i),
            Self::Real(f) => serde_json::json!(f),
            Self::Text(s) => serde_json::json!(s),
            Self::Blob(b) => serde_json::json!(b),
        }
    }
}

pub trait ToSql {
    fn to_sql(self) -> SqlValue;
}

impl ToSql for SqlValue {
    fn to_sql(self) -> SqlValue {
        self
    }
}

impl ToSql for () {
    fn to_sql(self) -> SqlValue {
        SqlValue::Null
    }
}

impl ToSql for bool {
    fn to_sql(self) -> SqlValue {
        SqlValue::Integer(i64::from(self))
    }
}

impl ToSql for i32 {
    fn to_sql(self) -> SqlValue {
        SqlValue::Integer(i64::from(self))
    }
}

impl ToSql for i64 {
    fn to_sql(self) -> SqlValue {
        SqlValue::Integer(self)
    }
}

impl ToSql for f64 {
    fn to_sql(self) -> SqlValue {
        SqlValue::Real(self)
    }
}

impl ToSql for String {
    fn to_sql(self) -> SqlValue {
        SqlValue::Text(self)
    }
}

impl ToSql for &str {
    fn to_sql(self) -> SqlValue {
        SqlValue::Text(self.to_string())
    }
}

impl ToSql for &String {
    fn to_sql(self) -> SqlValue {
        SqlValue::Text(self.clone())
    }
}

impl<T: ToSql> ToSql for Option<T> {
    fn to_sql(self) -> SqlValue {
        match self {
            Some(v) => v.to_sql(),
            None => SqlValue::Null,
        }
    }
}

pub trait IntoSqlParams {
    fn into_sql_params(self) -> Vec<SqlValue>;
}

impl IntoSqlParams for () {
    fn into_sql_params(self) -> Vec<SqlValue> {
        Vec::new()
    }
}

impl IntoSqlParams for Vec<SqlValue> {
    fn into_sql_params(self) -> Vec<SqlValue> {
        self
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    values: Vec<SqlValue>,
}

impl Row {
    pub fn new(values: Vec<SqlValue>) -> Self {
        Self { values }
    }

    pub fn get<T: FromSql>(&self, idx: usize) -> AppResult<T> {
        let value = self
            .values
            .get(idx)
            .ok_or_else(|| AppError::Database(format!("column index {idx} out of range")))?;
        T::from_sql(value)
    }
}

pub trait FromSql: Sized {
    fn from_sql(value: &SqlValue) -> AppResult<Self>;
}

impl FromSql for String {
    fn from_sql(value: &SqlValue) -> AppResult<Self> {
        match value {
            SqlValue::Text(s) => Ok(s.clone()),
            SqlValue::Integer(i) => Ok(i.to_string()),
            SqlValue::Real(f) => Ok(f.to_string()),
            SqlValue::Null => Err(AppError::Database("unexpected NULL for String".into())),
            SqlValue::Blob(_) => Err(AppError::Database("unexpected BLOB for String".into())),
        }
    }
}

impl FromSql for i32 {
    fn from_sql(value: &SqlValue) -> AppResult<Self> {
        Ok(i64::from_sql(value)? as i32)
    }
}

impl FromSql for i64 {
    fn from_sql(value: &SqlValue) -> AppResult<Self> {
        match value {
            SqlValue::Integer(i) => Ok(*i),
            SqlValue::Real(f) => Ok(*f as i64),
            SqlValue::Text(s) => s
                .parse()
                .map_err(|_| AppError::Database(format!("cannot parse i64 from {s:?}"))),
            SqlValue::Null => Err(AppError::Database("unexpected NULL for i64".into())),
            SqlValue::Blob(_) => Err(AppError::Database("unexpected BLOB for i64".into())),
        }
    }
}

impl FromSql for f64 {
    fn from_sql(value: &SqlValue) -> AppResult<Self> {
        match value {
            SqlValue::Real(f) => Ok(*f),
            SqlValue::Integer(i) => Ok(*i as f64),
            SqlValue::Text(s) => s
                .parse()
                .map_err(|_| AppError::Database(format!("cannot parse f64 from {s:?}"))),
            SqlValue::Null => Err(AppError::Database("unexpected NULL for f64".into())),
            SqlValue::Blob(_) => Err(AppError::Database("unexpected BLOB for f64".into())),
        }
    }
}

impl FromSql for bool {
    fn from_sql(value: &SqlValue) -> AppResult<Self> {
        match value {
            SqlValue::Integer(i) => Ok(*i != 0),
            SqlValue::Real(f) => Ok(*f != 0.0),
            SqlValue::Text(s) => Ok(s == "1" || s.eq_ignore_ascii_case("true")),
            SqlValue::Null => Ok(false),
            SqlValue::Blob(_) => Err(AppError::Database("unexpected BLOB for bool".into())),
        }
    }
}

impl<T: FromSql> FromSql for Option<T> {
    fn from_sql(value: &SqlValue) -> AppResult<Self> {
        match value {
            SqlValue::Null => Ok(None),
            other => Ok(Some(T::from_sql(other)?)),
        }
    }
}

/// Map a decoded row into a typed value.
pub trait FromRow: Sized {
    fn from_row(row: &Row) -> AppResult<Self>;
}

impl FromRow for (String,) {
    fn from_row(row: &Row) -> AppResult<Self> {
        Ok((row.get(0)?,))
    }
}

impl FromRow for (String, String) {
    fn from_row(row: &Row) -> AppResult<Self> {
        Ok((row.get(0)?, row.get(1)?))
    }
}

impl FromRow for (String, String, String) {
    fn from_row(row: &Row) -> AppResult<Self> {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    }
}

impl FromRow for (i32, String) {
    fn from_row(row: &Row) -> AppResult<Self> {
        Ok((row.get(0)?, row.get(1)?))
    }
}

#[macro_export]
macro_rules! params {
    () => {
        Vec::<$crate::db::SqlValue>::new()
    };
    ($($value:expr),+ $(,)?) => {
        vec![$($crate::db::ToSql::to_sql($value)),+]
    };
}

pub use crate::params;
