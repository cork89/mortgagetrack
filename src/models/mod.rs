mod amort;
mod db;
mod migrate;
mod sharing;
mod view;

pub use amort::build_schedule;
pub use db::{
    add_extra, clear_paid, create_profile, delete_extra, delete_profile, extras_as_inputs,
    get_active_profile_id, list_extras, list_paid_keys, list_payment_notes, list_profiles,
    load_profile, mark_due_paid, rename_profile, require_profile_access, set_active_profile,
    toggle_paid, update_profile_loan, upsert_payment_note, ExtraPayment, ProfileRole,
};
pub use migrate::{ensure_extra_recast, ensure_profile_version, ensure_profiles_belong_to_users};
pub use sharing::{
    accept_share_link, active_share_link, create_share_link, leave_profile, list_collaborators,
    remove_collaborator, CollaboratorRow, ShareLinkStatus,
};
pub use view::{
    build_dashboard, empty_state, ChartPair, DashboardView, EmptyState, MonthCell, PaymentChip,
    PaymentFilter, PaymentRowView, ProfileOption, TabId, YearStat, YearSummary,
};
