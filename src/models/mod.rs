mod amort;
mod db;
mod export;
mod migrate;
mod sharing;
mod view;

pub use amort::build_schedule;
pub use db::{
    add_extra, add_improvement, clear_paid, count_owned_profiles, create_profile, delete_extra,
    delete_improvement, delete_profile, extras_as_inputs, get_active_profile_id, list_extras,
    list_paid_keys, list_payment_notes, list_profiles, load_page_bundle, load_profile,
    mark_due_paid, rename_profile, require_profile_access, set_active_profile, set_paid,
    update_improvement, update_profile_loan, upsert_payment_note, ExtraPayment, ProfileRole,
};
pub use export::{csv_filename_stem, payments_csv};
pub use migrate::{
    ensure_extra_recast, ensure_improvement_detail, ensure_profile_version,
    ensure_profiles_belong_to_users, ensure_user_avatar, ensure_user_default_tab,
    ensure_user_payments_year_expand, ensure_user_paid_until, ensure_user_role,
    ensure_user_tier,
};
pub use sharing::{
    accept_share_link, active_share_link, create_share_link, leave_profile, list_collaborators,
    remove_collaborator, CollaboratorRow, ShareLinkStatus,
};
pub use view::{
    build_dashboard, empty_state, ChartPair, DashboardView, EmptyState, ImprovementRowView,
    MonthCell, PaymentFilter, PaymentYearGroup, PaymentsYearExpand, PayoffAccelerator,
    ProfileOption, TabId, YearStat, YearSummary,
};
