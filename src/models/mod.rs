mod amort;
mod db;
mod view;

pub use amort::build_schedule;
pub use db::{
    add_extra, clear_paid, create_profile, delete_extra, delete_profile, get_active_profile_id,
    list_extras, list_paid_keys, list_profiles, load_profile, mark_due_paid, rename_profile,
    set_active_profile, toggle_paid, update_profile_loan, ExtraPayment, Loan, Profile,
};
pub use view::{
    build_dashboard, empty_state, money, ChartBucket, ChartView, DashboardView, EmptyState,
    MonthCell, PaymentChip, PaymentFilter, PaymentRowView, ProfileOption, TabId, YearStat,
    YearSummary,
};
