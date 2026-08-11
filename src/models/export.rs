//! Payment schedule CSV export (pro feature).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use chrono::{Datelike, NaiveDate};

use super::amort::{RowKind, ScheduleRow};
use super::view::PaymentFilter;

/// Build a CSV document for the payment schedule (header + filtered rows).
pub fn payments_csv(
    rows: &[ScheduleRow],
    paid: &HashSet<String>,
    notes: &HashMap<String, String>,
    filter: PaymentFilter,
    today: NaiveDate,
) -> String {
    let mut out =
        String::from("label,due_date,type,payment,principal,interest,balance,status,note\n");
    for row in rows
        .iter()
        .filter(|row| row_matches_filter(row, paid, filter, today))
    {
        let is_paid = paid.contains(&row.pay_key);
        let note = notes.get(&row.pay_key).map(String::as_str).unwrap_or("");
        let _ = writeln!(
            out,
            "{},{},{},{},{},{},{},{},{}",
            csv_escape(&row.label),
            row.due.format("%Y-%m-%d"),
            row_type(row),
            format_amount(row.payment),
            format_amount(row.principal),
            format_amount(row.interest),
            format_amount(row.balance),
            if is_paid { "paid" } else { "unpaid" },
            csv_escape(note),
        );
    }
    out
}

fn row_matches_filter(
    row: &ScheduleRow,
    paid: &HashSet<String>,
    filter: PaymentFilter,
    today: NaiveDate,
) -> bool {
    let is_paid = paid.contains(&row.pay_key);
    match filter {
        PaymentFilter::Paid => is_paid,
        PaymentFilter::Unpaid => !is_paid,
        PaymentFilter::Year => row.due.year() == today.year(),
        PaymentFilter::Extra => row.kind == RowKind::Extra,
        PaymentFilter::All => true,
    }
}

fn row_type(row: &ScheduleRow) -> &'static str {
    match row.kind {
        RowKind::Scheduled => "scheduled",
        RowKind::Extra if row.recast => "recast",
        RowKind::Extra => "extra",
    }
}

fn format_amount(v: f64) -> String {
    // Stable, locale-free decimal; trim noisy float tails for clean CSV.
    let s = format!("{v:.2}");
    s
}

/// RFC 4180-ish field escaping: quote when needed; double internal quotes.
pub fn csv_escape(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    let needs_quotes = value.contains(['"', ',', '\n', '\r']);
    if !needs_quotes {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

/// Safe Content-Disposition filename stem from a profile name.
pub fn csv_filename_stem(profile_name: &str) -> String {
    let mut stem = String::new();
    for ch in profile_name.chars() {
        if ch.is_ascii_alphanumeric() {
            stem.push(ch.to_ascii_lowercase());
        } else if matches!(ch, ' ' | '-' | '_') && !stem.ends_with('-') {
            stem.push('-');
        }
    }
    let stem = stem.trim_matches('-');
    if stem.is_empty() {
        "payments".into()
    } else {
        format!("payments-{stem}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn row(
        label: &str,
        due: &str,
        kind: RowKind,
        pay_key: &str,
        payment: f64,
        recast: bool,
    ) -> ScheduleRow {
        ScheduleRow {
            kind,
            label: label.into(),
            id: None,
            due: NaiveDate::parse_from_str(due, "%Y-%m-%d").unwrap(),
            pay_key: pay_key.into(),
            payment,
            principal: payment * 0.6,
            interest: payment * 0.4,
            balance: 100_000.0,
            recast,
        }
    }

    #[test]
    fn csv_escape_quotes_and_commas() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn filename_stem_sanitizes() {
        assert_eq!(csv_filename_stem("My Home"), "payments-my-home");
        assert_eq!(csv_filename_stem("!!!"), "payments");
        assert_eq!(csv_filename_stem("A/B Condo"), "payments-ab-condo");
    }

    #[test]
    fn payments_csv_filters_and_types() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
        let rows = vec![
            row(
                "1",
                "2026-01-01",
                RowKind::Scheduled,
                "2026-01-01",
                1000.0,
                false,
            ),
            row(
                "2",
                "2026-02-01",
                RowKind::Scheduled,
                "2026-02-01",
                1000.0,
                false,
            ),
            row(
                "Extra",
                "2026-02-15",
                RowKind::Extra,
                "extra:abc",
                5000.0,
                false,
            ),
            row(
                "Recast",
                "2025-12-01",
                RowKind::Extra,
                "extra:rec",
                2000.0,
                true,
            ),
        ];
        let mut paid = HashSet::new();
        paid.insert("2026-01-01".into());
        let mut notes = HashMap::new();
        notes.insert("2026-02-01".into(), "bonus, \"tax\"".into());

        let all = payments_csv(&rows, &paid, &notes, PaymentFilter::All, today);
        assert!(
            all.starts_with("label,due_date,type,payment,principal,interest,balance,status,note\n")
        );
        assert!(all.contains("1,2026-01-01,scheduled,1000.00,600.00,400.00,100000.00,paid,"));
        assert!(all.contains(
            "2,2026-02-01,scheduled,1000.00,600.00,400.00,100000.00,unpaid,\"bonus, \"\"tax\"\"\""
        ));
        assert!(all.contains("Extra,2026-02-15,extra,5000.00,"));
        assert!(all.contains("Recast,2025-12-01,recast,2000.00,"));

        let paid_only = payments_csv(&rows, &paid, &notes, PaymentFilter::Paid, today);
        assert!(paid_only.contains("2026-01-01"));
        assert!(!paid_only.contains("2026-02-01"));

        let year = payments_csv(&rows, &paid, &notes, PaymentFilter::Year, today);
        assert!(year.contains("2026-01-01"));
        assert!(!year.contains("2025-12-01"));

        let extras = payments_csv(&rows, &paid, &notes, PaymentFilter::Extra, today);
        assert!(extras.contains(",extra,"));
        assert!(extras.contains(",recast,"));
        assert!(!extras.contains(",scheduled,"));
    }
}
