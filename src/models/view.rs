use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Month, NaiveDate};

use super::amort::{build_schedule, payment_status, RowKind, ScheduleRow};
use super::db::{extras_as_inputs, ExtraPayment, HomeImprovement, Loan, Profile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabId {
    Summary,
    Calendar,
    Payments,
    Improvements,
    Chart,
}

impl TabId {
    pub const ALL: [TabId; 5] = [
        TabId::Summary,
        TabId::Calendar,
        TabId::Payments,
        TabId::Improvements,
        TabId::Chart,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            TabId::Summary => "summary",
            TabId::Calendar => "calendar",
            TabId::Payments => "payments",
            TabId::Improvements => "improvements",
            TabId::Chart => "chart",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TabId::Summary => "Summary",
            TabId::Calendar => "Calendar",
            TabId::Payments => "Payments",
            TabId::Improvements => "Improvements",
            TabId::Chart => "Breakdown",
        }
    }

    pub fn try_parse(s: &str) -> Option<Self> {
        match s {
            "summary" => Some(TabId::Summary),
            "calendar" => Some(TabId::Calendar),
            "payments" => Some(TabId::Payments),
            "improvements" => Some(TabId::Improvements),
            "chart" => Some(TabId::Chart),
            _ => None,
        }
    }

    pub fn parse(s: &str) -> Self {
        Self::try_parse(s).unwrap_or(TabId::Calendar)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentFilter {
    All,
    Unpaid,
    Paid,
    Year,
    Extra,
}

impl PaymentFilter {
    pub fn parse(s: &str) -> Self {
        match s {
            "unpaid" => PaymentFilter::Unpaid,
            "paid" => PaymentFilter::Paid,
            "year" => PaymentFilter::Year,
            "extra" => PaymentFilter::Extra,
            _ => PaymentFilter::All,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PaymentFilter::All => "all",
            PaymentFilter::Unpaid => "unpaid",
            PaymentFilter::Paid => "paid",
            PaymentFilter::Year => "year",
            PaymentFilter::Extra => "extra",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProfileOption {
    pub id: String,
    pub name: String,
    pub selected: bool,
    pub is_shared: bool,
}

#[derive(Debug, Clone)]
pub struct EmptyState {
    pub title: String,
    pub copy: String,
    pub button_label: String,
    pub action: String,
}

#[derive(Debug, Clone)]
pub struct YearStat {
    pub label: String,
    pub value: String,
    pub sub: String,
    pub class: String,
}

#[derive(Debug, Clone)]
pub struct PaymentChip {
    pub pay_key: String,
    pub amount: String,
    pub status: String,
    pub status_text: String,
    pub is_extra: bool,
    pub aria: String,
}

#[derive(Debug, Clone)]
pub struct MonthCell {
    pub name: String,
    pub is_current: bool,
    pub is_empty: bool,
    pub chips: Vec<PaymentChip>,
}

#[derive(Debug, Clone)]
pub struct PaymentRowView {
    pub is_year_header: bool,
    pub year_label: String,
    pub is_current_year: bool,
    pub is_current_month: bool,
    pub label_html: String,
    pub due: String,
    pub payment: String,
    pub principal: String,
    pub interest: String,
    pub balance: String,
    pub balance_short: String,
    pub pay_key: String,
    pub paid: bool,
    pub is_extra: bool,
    pub extra_id: String,
    pub zeroed: bool,
    pub has_note: bool,
    pub note_json: String,
}

#[derive(Debug, Clone)]
pub struct YearSummary {
    pub monthly_payment: String,
    pub total_interest: String,
    pub balance_after: String,
    pub hint: String,
}

#[derive(Debug, Clone)]
pub struct ImprovementRowView {
    pub id: String,
    pub date: String,
    pub date_raw: String,
    pub amount: String,
    pub amount_raw: String,
    pub note: String,
    pub has_note: bool,
    pub note_json: String,
    pub has_detail: bool,
    pub detail_json: String,
}

#[derive(Debug, Clone)]
pub struct PayoffAccelerator {
    pub remaining: String,
    pub paid_label: String,
    pub extra_label: String,
    pub saved_label: String,
    pub interest_label: String,
    pub show_interest: bool,
    pub paid_width: String,
    pub extra_width: String,
    pub bar_aria: String,
}

#[derive(Debug, Clone)]
pub struct ChartBucket {
    pub label: String,
    pub principal: f64,
    pub interest: f64,
    pub payment: f64,
    pub year: i32,
    pub count: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ChartView {
    pub hint: String,
    pub buckets_json: String,
}

/// Both chart grains, so the breakdown toggle can switch without a network round-trip.
#[derive(Debug, Clone)]
pub struct ChartPair {
    pub grain: String,
    pub monthly: ChartView,
    pub yearly: ChartView,
}

#[derive(Debug, Clone)]
pub struct DashboardView {
    pub profile_id: String,
    pub profile_version: i64,
    pub profile_principal: String,
    pub profile_rate: String,
    pub profile_term: String,
    pub accelerator: PayoffAccelerator,
    pub year_stats: Vec<YearStat>,
    pub view_year: i32,
    pub months: Vec<MonthCell>,
    pub payment_rows: Vec<PaymentRowView>,
    pub payment_filter: String,
    pub summary: YearSummary,
    pub chart: ChartPair,
    pub active_tab: String,
    pub loan_principal: String,
    pub loan_rate: String,
    pub loan_term: String,
    pub loan_start: String,
    pub profile_name: String,
    pub extra_date_default: String,
    pub improvements: Vec<ImprovementRowView>,
    pub improvements_total: String,
}

pub fn money(n: f64) -> String {
    let neg = n < 0.0;
    let n = n.abs();
    let whole = n.floor() as i64;
    let cents = ((n - whole as f64) * 100.0).round() as i64;
    let s = format!("{whole}");
    let bytes = s.as_bytes().to_vec();
    let mut out = String::new();
    for (i, ch) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*ch as char);
    }
    let formatted = format!("${out}.{:02}", cents.clamp(0, 99));
    if neg {
        format!("-{formatted}")
    } else {
        formatted
    }
}

fn money_short(n: f64) -> String {
    let neg = n < 0.0;
    let abs = n.abs();
    let formatted = if abs >= 1000.0 {
        format!("${:.0}k", abs / 1000.0)
    } else {
        money(abs)
    };
    if neg {
        format!("-{formatted}")
    } else {
        formatted
    }
}

fn fmt_date(d: NaiveDate) -> String {
    let month = Month::try_from(d.month() as u8)
        .map(|m| m.name())
        .unwrap_or("?");
    let short = &month[..3.min(month.len())];
    format!("{short} {}, {}", d.day(), d.year())
}

fn status_label(status: &str) -> &'static str {
    match status {
        "paid" => "Paid",
        "due" => "Due this month",
        "missed" => "Past due",
        _ => "Upcoming",
    }
}

fn chip_for(row: &ScheduleRow, paid: &HashSet<String>, today: NaiveDate) -> PaymentChip {
    let is_paid = paid.contains(&row.pay_key);
    let status = payment_status(row.due, is_paid, today).to_string();
    let status_text = if row.kind == RowKind::Extra {
        let kind = if row.recast { "Recast" } else { "Extra" };
        if status == "paid" {
            format!("{kind} · Paid")
        } else {
            format!("{kind} · {}", status_label(&status))
        }
    } else {
        status_label(&status).to_string()
    };
    let aria = if status == "paid" {
        "Mark unpaid"
    } else {
        "Mark paid"
    };
    PaymentChip {
        pay_key: row.pay_key.clone(),
        amount: money(row.payment),
        status,
        status_text,
        is_extra: row.kind == RowKind::Extra,
        aria: aria.to_string(),
    }
}

pub fn build_dashboard(
    profile: &Profile,
    paid_keys: &[String],
    extras: &[ExtraPayment],
    notes: &HashMap<String, String>,
    improvements: &[HomeImprovement],
    view_year: i32,
    filter: PaymentFilter,
    chart_grain: &str,
    active_tab: TabId,
    today: NaiveDate,
) -> Option<DashboardView> {
    let loan = profile.loan()?;
    let paid: HashSet<String> = paid_keys.iter().cloned().collect();
    let extra_inputs = extras_as_inputs(extras, &paid);

    let built = build_schedule(
        loan.principal,
        loan.rate,
        loan.term_years,
        loan.start_date,
        &extra_inputs,
    );
    let schedule = &built.rows;

    let year_stats = year_strip(schedule, &paid, extras.len(), loan.principal, today);
    let accelerator = payoff_accelerator(schedule, &paid, extras, &loan);
    let months = calendar_months(schedule, &paid, view_year, today);
    let (payment_rows, summary) = payments_table(
        schedule,
        &paid,
        notes,
        loan.principal,
        built.payment,
        built.total_interest,
        filter,
        today,
    );
    let chart = ChartPair {
        grain: if chart_grain == "yearly" {
            "yearly".into()
        } else {
            "monthly".into()
        },
        monthly: build_chart(schedule, "monthly"),
        yearly: build_chart(schedule, "yearly"),
    };

    let improvements_total: f64 = improvements.iter().map(|i| i.amount).sum();
    let improvement_rows: Vec<ImprovementRowView> = improvements
        .iter()
        .map(|i| {
            let date = NaiveDate::parse_from_str(&i.date, "%Y-%m-%d")
                .map(fmt_date)
                .unwrap_or_else(|_| i.date.clone());
            let note = i.note.trim().to_string();
            let detail = i.detail.trim().to_string();
            let note_json = serde_json::to_string(&note).unwrap_or_else(|_| "\"\"".into());
            let detail_json = serde_json::to_string(&detail).unwrap_or_else(|_| "\"\"".into());
            ImprovementRowView {
                id: i.id.clone(),
                date,
                date_raw: i.date.clone(),
                amount: money(i.amount),
                amount_raw: format!("{:.2}", i.amount),
                has_note: !note.is_empty(),
                note,
                note_json,
                has_detail: !detail.is_empty(),
                detail_json,
            }
        })
        .collect();

    Some(DashboardView {
        profile_id: profile.id.clone(),
        profile_version: profile.version,
        profile_principal: money(loan.principal),
        profile_rate: format!("{}%", loan.rate),
        profile_term: format!("{}yr", loan.term_years),
        accelerator,
        year_stats,
        view_year,
        months,
        payment_rows,
        payment_filter: filter.as_str().to_string(),
        summary,
        chart,
        active_tab: active_tab.as_str().to_string(),
        loan_principal: format!("{}", loan.principal as i64),
        loan_rate: format!("{}", loan.rate),
        loan_term: loan.term_years.to_string(),
        loan_start: loan.start_date.format("%Y-%m-%d").to_string(),
        profile_name: profile.name.clone(),
        extra_date_default: today.format("%Y-%m-%d").to_string(),
        improvements: improvement_rows,
        improvements_total: money(improvements_total),
    })
}

pub fn empty_state(profile: Option<&Profile>) -> EmptyState {
    if profile.is_some() {
        EmptyState {
            title: "Add loan details".into(),
            copy: "Edit this profile with your loan amount, rate, and first payment date.".into(),
            button_label: "Edit profile".into(),
            action: "edit".into(),
        }
    } else {
        EmptyState {
            title: "No mortgage yet".into(),
            copy: "Create a profile with your loan details to see the schedule and calendar."
                .into(),
            button_label: "New profile".into(),
            action: "create".into(),
        }
    }
}

fn money_whole(n: f64) -> String {
    let neg = n < 0.0;
    let whole = n.abs().round() as i64;
    let s = format!("{whole}");
    let bytes = s.as_bytes();
    let mut out = String::new();
    for (i, ch) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*ch as char);
    }
    let formatted = format!("${out}");
    if neg {
        format!("-{formatted}")
    } else {
        formatted
    }
}

fn months_between(earlier: NaiveDate, later: NaiveDate) -> i32 {
    (later.year() - earlier.year()) * 12 + (later.month() as i32 - earlier.month() as i32)
}

fn last_active_due(schedule: &[ScheduleRow]) -> Option<NaiveDate> {
    schedule
        .iter()
        .rev()
        .find(|r| r.kind == RowKind::Scheduled && r.payment > 0.005)
        .map(|r| r.due)
}

fn format_years_saved(months: i32) -> String {
    if months <= 0 {
        return "0 Yrs Saved".into();
    }
    if months < 12 {
        return format!("{months} Mo Saved");
    }
    let years = months as f64 / 12.0;
    if (years - years.round()).abs() < 0.05 {
        format!("{:.0} Yrs Saved", years.round())
    } else {
        format!("{:.1} Yrs Saved", years)
    }
}

fn format_extra_label(extras: &[ExtraPayment]) -> String {
    if extras.is_empty() {
        return "No extras".into();
    }

    let regular: Vec<f64> = extras
        .iter()
        .filter(|e| !e.recast)
        .map(|e| e.amount)
        .collect();
    let recast_total: f64 = extras.iter().filter(|e| e.recast).map(|e| e.amount).sum();
    let recast_count = extras.iter().filter(|e| e.recast).count();

    if regular.is_empty() {
        let label = if recast_count == 1 {
            "Recast"
        } else {
            "Recasts"
        };
        return format!("+{} {label}", money_whole(recast_total));
    }

    let mut freq: HashMap<i64, usize> = HashMap::new();
    for amount in &regular {
        *freq.entry(amount.round() as i64).or_default() += 1;
    }
    let extra_bit = if let Some((&amt, &count)) = freq.iter().max_by_key(|(_, c)| *c) {
        if count >= 2 {
            format!("+{}/mo Extra", money_whole(amt as f64))
        } else {
            format!("+{} Extra", money_whole(regular.iter().sum()))
        }
    } else {
        format!("+{} Extra", money_whole(regular.iter().sum()))
    };

    if recast_count == 0 {
        extra_bit
    } else {
        format!("{extra_bit} · {} Recast", money_whole(recast_total))
    }
}

fn payoff_accelerator(
    schedule: &[ScheduleRow],
    paid: &HashSet<String>,
    extras: &[ExtraPayment],
    loan: &Loan,
) -> PayoffAccelerator {
    let mut remaining = loan.principal;
    for row in schedule {
        if paid.contains(&row.pay_key) {
            remaining = row.balance;
        } else {
            break;
        }
    }

    let paid_count = schedule
        .iter()
        .filter(|r| paid.contains(&r.pay_key))
        .count();
    let scheduled_paid: f64 = schedule
        .iter()
        .filter(|r| r.kind == RowKind::Scheduled && paid.contains(&r.pay_key))
        .map(|r| r.principal)
        .sum();
    let extra_paid: f64 = schedule
        .iter()
        .filter(|r| r.kind == RowKind::Extra && paid.contains(&r.pay_key))
        .map(|r| r.principal)
        .sum();

    let denom = loan.principal.max(1.0);
    let mut paid_pct = ((scheduled_paid / denom) * 100.0).clamp(0.0, 100.0);
    let mut extra_pct = ((extra_paid / denom) * 100.0).clamp(0.0, 100.0);
    if paid_pct + extra_pct > 100.0 {
        extra_pct = (100.0 - paid_pct).max(0.0);
    }
    // Keep a visible sliver once there's any progress.
    if scheduled_paid > 0.0 && paid_pct < 1.5 {
        paid_pct = 1.5;
    }
    if extra_paid > 0.0 && extra_pct < 1.5 {
        extra_pct = (100.0 - paid_pct).min(1.5);
    }

    let baseline = build_schedule(
        loan.principal,
        loan.rate,
        loan.term_years,
        loan.start_date,
        &[],
    );
    let months_saved = match (last_active_due(&baseline.rows), last_active_due(schedule)) {
        (Some(base_end), Some(accel_end)) if accel_end < base_end => {
            months_between(accel_end, base_end).max(0)
        }
        _ => 0,
    };
    let current_interest: f64 = schedule.iter().map(|r| r.interest).sum();
    let interest_saved = (baseline.total_interest - current_interest).max(0.0);

    let paid_label = format!("{paid_count} Paid");
    let extra_label = format_extra_label(extras);
    let saved_label = format_years_saved(months_saved);
    let show_interest = interest_saved >= 1.0;
    let interest_label = if show_interest {
        format!("{} Interest Saved", money_short(interest_saved))
    } else {
        String::new()
    };
    let bar_aria = format!(
        "{:.0}% of principal paid through scheduled payments, {:.0}% through extras",
        paid_pct, extra_pct
    );

    PayoffAccelerator {
        remaining: money_whole(remaining),
        paid_label,
        extra_label,
        saved_label,
        interest_label,
        show_interest,
        paid_width: format!("{paid_pct:.1}%"),
        extra_width: format!("{extra_pct:.1}%"),
        bar_aria,
    }
}

fn year_strip(
    schedule: &[ScheduleRow],
    paid: &HashSet<String>,
    extra_count: usize,
    original_principal: f64,
    today: NaiveDate,
) -> Vec<YearStat> {
    let y = today.year();
    let year_rows: Vec<_> = schedule.iter().filter(|r| r.due.year() == y).collect();
    let paid_rows: Vec<_> = year_rows
        .iter()
        .filter(|r| paid.contains(&r.pay_key))
        .collect();
    let scheduled_only: Vec<_> = year_rows
        .iter()
        .filter(|r| r.kind == RowKind::Scheduled)
        .collect();
    let scheduled_principal: f64 = scheduled_only.iter().map(|r| r.principal).sum();
    let scheduled_interest: f64 = year_rows.iter().map(|r| r.interest).sum();
    let paid_principal: f64 = paid_rows.iter().map(|r| r.principal).sum();
    let paid_interest: f64 = paid_rows.iter().map(|r| r.interest).sum();
    let remaining = year_rows.len() - paid_rows.len();

    let total_paid_principal: f64 = schedule
        .iter()
        .filter(|r| paid.contains(&r.pay_key))
        .map(|r| r.principal)
        .sum();
    let pct_paid = if original_principal <= 0.0 {
        0
    } else {
        ((total_paid_principal / original_principal) * 100.0)
            .clamp(0.0, 100.0)
            .round() as u32
    };

    let mut extra_note = String::new();
    if extra_count > 0 {
        extra_note = format!(" · {extra_count} extra");
    }

    vec![
        YearStat {
            label: "Total % paid".into(),
            value: format!("{pct_paid}%"),
            sub: format!(
                "{} of {} principal",
                money(total_paid_principal),
                money(original_principal)
            ),
            class: "stat highlight".into(),
        },
        YearStat {
            label: format!("{y} progress"),
            value: format!("{} / {}", paid_rows.len(), year_rows.len()),
            sub: format!("{remaining} remaining{extra_note}"),
            class: "stat next-up".into(),
        },
        YearStat {
            label: format!("{y} principal"),
            value: money(paid_principal),
            sub: format!("of {} scheduled", money(scheduled_principal)),
            class: "stat quiet".into(),
        },
        YearStat {
            label: format!("{y} interest"),
            value: money(paid_interest),
            sub: format!("of {} scheduled", money(scheduled_interest)),
            class: "stat quiet".into(),
        },
    ]
}

fn calendar_months(
    schedule: &[ScheduleRow],
    paid: &HashSet<String>,
    view_year: i32,
    today: NaiveDate,
) -> Vec<MonthCell> {
    let mut by_month: HashMap<u32, Vec<&ScheduleRow>> = HashMap::new();
    for row in schedule {
        if row.due.year() != view_year {
            continue;
        }
        by_month.entry(row.due.month()).or_default().push(row);
    }

    (1..=12)
        .map(|month| {
            let name = Month::try_from(month as u8)
                .map(|m| m.name().to_string())
                .unwrap_or_else(|_| month.to_string());
            let rows = by_month.get(&month).cloned().unwrap_or_default();
            let is_current = today.year() == view_year && today.month() == month;
            let chips = rows
                .iter()
                .filter(|r| r.payment >= 0.005 || r.kind == RowKind::Extra)
                .map(|r| chip_for(r, paid, today))
                .collect::<Vec<_>>();
            MonthCell {
                name,
                is_current,
                is_empty: chips.is_empty(),
                chips,
            }
        })
        .collect()
}

fn note_fields(notes: &HashMap<String, String>, pay_key: &str) -> (bool, String) {
    let note = notes.get(pay_key).cloned().unwrap_or_default();
    let has_note = !note.trim().is_empty();
    let note_json = serde_json::to_string(&note).unwrap_or_else(|_| "\"\"".into());
    (has_note, note_json)
}

fn payments_table(
    schedule: &[ScheduleRow],
    paid: &HashSet<String>,
    notes: &HashMap<String, String>,
    principal: f64,
    monthly_payment: f64,
    total_interest: f64,
    filter: PaymentFilter,
    today: NaiveDate,
) -> (Vec<PaymentRowView>, YearSummary) {
    let y = today.year();
    let rows: Vec<&ScheduleRow> = schedule
        .iter()
        .filter(|row| {
            let is_paid = paid.contains(&row.pay_key);
            match filter {
                PaymentFilter::Paid => is_paid,
                PaymentFilter::Unpaid => !is_paid,
                PaymentFilter::Year => row.due.year() == y,
                PaymentFilter::Extra => row.kind == RowKind::Extra,
                PaymentFilter::All => true,
            }
        })
        .collect();

    let current_month_key = rows
        .iter()
        .find(|r| {
            r.kind == RowKind::Scheduled
                && r.due.year() == today.year()
                && r.due.month() == today.month()
        })
        .or_else(|| {
            rows.iter()
                .find(|r| r.due.year() == today.year() && r.due.month() == today.month())
        })
        .map(|r| r.pay_key.as_str());

    let mut out = Vec::new();
    if rows.is_empty() {
        out.push(PaymentRowView {
            is_year_header: false,
            year_label: String::new(),
            is_current_year: false,
            is_current_month: false,
            label_html: String::new(),
            due: String::new(),
            payment: String::new(),
            principal: String::new(),
            interest: String::new(),
            balance: String::new(),
            balance_short: String::new(),
            pay_key: String::new(),
            paid: false,
            is_extra: false,
            extra_id: String::new(),
            zeroed: false,
            has_note: false,
            note_json: "\"\"".into(),
        });
        // empty marker handled in template via empty check on payment_rows length + special flag
    } else {
        let mut groups: Vec<(i32, Vec<&ScheduleRow>)> = Vec::new();
        for row in rows {
            if groups.last().map(|(yr, _)| *yr) != Some(row.due.year()) {
                groups.push((row.due.year(), Vec::new()));
            }
            groups.last_mut().unwrap().1.push(row);
        }

        for (year, group) in groups {
            let payment_total: f64 = group.iter().map(|r| r.payment).sum();
            let principal_total: f64 = group.iter().map(|r| r.principal).sum();
            let interest_total: f64 = group.iter().map(|r| r.interest).sum();
            out.push(PaymentRowView {
                is_year_header: true,
                year_label: year.to_string(),
                is_current_year: year == y,
                is_current_month: false,
                label_html: String::new(),
                due: String::new(),
                payment: money(payment_total),
                principal: money(principal_total),
                interest: money(interest_total),
                balance: String::new(),
                balance_short: String::new(),
                pay_key: String::new(),
                paid: false,
                is_extra: false,
                extra_id: String::new(),
                zeroed: false,
                has_note: false,
                note_json: "\"\"".into(),
            });

            for row in group {
                let is_paid = paid.contains(&row.pay_key);
                let label_html = if row.kind == RowKind::Extra {
                    if row.recast {
                        r#"<span class="type-pill">Recast</span>"#.to_string()
                    } else {
                        r#"<span class="type-pill">Extra</span>"#.to_string()
                    }
                } else {
                    row.label.clone()
                };
                let (has_note, note_json) = note_fields(notes, &row.pay_key);
                out.push(PaymentRowView {
                    is_year_header: false,
                    year_label: String::new(),
                    is_current_year: row.due.year() == y,
                    is_current_month: current_month_key == Some(row.pay_key.as_str()),
                    label_html,
                    due: fmt_date(row.due),
                    payment: money(row.payment),
                    principal: money(row.principal),
                    interest: money(row.interest),
                    balance: money(row.balance),
                    balance_short: money_short(row.balance),
                    pay_key: row.pay_key.clone(),
                    paid: is_paid,
                    is_extra: row.kind == RowKind::Extra,
                    extra_id: row.id.clone().unwrap_or_default(),
                    zeroed: row.payment < 0.005,
                    has_note,
                    note_json,
                });
            }
        }
    }

    let mut bal = principal;
    for r in schedule {
        if paid.contains(&r.pay_key) {
            bal = r.balance;
        } else {
            break;
        }
    }
    let scheduled_count = schedule
        .iter()
        .filter(|r| r.kind == RowKind::Scheduled)
        .count();
    let paid_count = schedule
        .iter()
        .filter(|r| paid.contains(&r.pay_key))
        .count();
    let pct_paid = if schedule.is_empty() {
        0
    } else {
        ((paid_count as f64 / schedule.len() as f64) * 100.0).round() as u32
    };

    let summary = YearSummary {
        monthly_payment: money(monthly_payment),
        total_interest: money(total_interest),
        balance_after: money(bal),
        hint: format!("{scheduled_count} scheduled · {pct_paid}% paid"),
    };

    // Fix empty marker: use empty payment_rows when no matches
    if schedule.is_empty()
        || (filter != PaymentFilter::All
            && out.len() == 1
            && !out[0].is_year_header
            && out[0].pay_key.is_empty())
    {
        // replace with empty vec so template shows empty state
        if out.len() == 1 && out[0].pay_key.is_empty() && !out[0].is_year_header {
            out.clear();
        }
    }

    (out, summary)
}

fn build_chart(schedule: &[ScheduleRow], grain: &str) -> ChartView {
    let months: Vec<&ScheduleRow> = schedule
        .iter()
        .filter(|r| r.kind == RowKind::Scheduled)
        .collect();

    let buckets: Vec<ChartBucket> = if grain == "yearly" {
        let mut by_year: HashMap<i32, ChartBucket> = HashMap::new();
        for row in &months {
            let year = row.due.year();
            let entry = by_year.entry(year).or_insert(ChartBucket {
                label: year.to_string(),
                principal: 0.0,
                interest: 0.0,
                payment: 0.0,
                year,
                count: Some(0),
            });
            entry.principal += row.principal;
            entry.interest += row.interest;
            entry.payment += row.payment;
            entry.count = Some(entry.count.unwrap_or(0) + 1);
        }
        let mut v: Vec<_> = by_year.into_values().collect();
        v.sort_by_key(|b| b.year);
        v
    } else {
        months
            .iter()
            .map(|row| ChartBucket {
                label: fmt_date(row.due),
                principal: row.principal,
                interest: row.interest,
                payment: row.payment,
                year: row.due.year(),
                count: None,
            })
            .collect()
    };

    let total_principal: f64 = buckets.iter().map(|b| b.principal).sum();
    let total_interest: f64 = buckets.iter().map(|b| b.interest).sum();
    let hint = if grain == "yearly" {
        format!(
            "{} years · {} principal · {} interest",
            buckets.len(),
            money(total_principal),
            money(total_interest)
        )
    } else {
        format!(
            "{} payments · {} principal · {} interest",
            buckets.len(),
            money(total_principal),
            money(total_interest)
        )
    };

    let buckets_json = serde_json::to_string(
        &buckets
            .iter()
            .map(|b| {
                serde_json::json!({
                    "label": b.label,
                    "year": b.year,
                    "principal": b.principal,
                    "interest": b.interest,
                    "payment": b.payment,
                    "count": b.count,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());

    ChartView {
        hint,
        buckets_json,
    }
}
