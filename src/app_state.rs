use chrono::NaiveDate;

use crate::db::DbPool;
use crate::mail::Mailer;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    /// When set (via `CURRENT_DATE=YYYY-MM-DD`), replaces the system date for schedule UI.
    pub today_override: Option<NaiveDate>,
    pub mailer: Mailer,
    /// Public origin for emailed links (no trailing slash).
    pub app_base_url: String,
    /// Product name shown in HTML (from `APP_NAME`, default MortgageTrack).
    pub app_name: String,
}

impl AppState {
    pub fn today(&self) -> NaiveDate {
        self.today_override
            .unwrap_or_else(|| chrono::Local::now().date_naive())
    }
}
