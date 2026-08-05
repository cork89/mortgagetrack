use chrono::NaiveDate;

use crate::db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    /// When set (via `CURRENT_DATE=YYYY-MM-DD`), replaces the system date for schedule UI.
    pub today_override: Option<NaiveDate>,
}

impl AppState {
    pub fn today(&self) -> NaiveDate {
        self.today_override
            .unwrap_or_else(|| chrono::Local::now().date_naive())
    }
}
