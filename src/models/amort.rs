use chrono::{Datelike, NaiveDate};

#[derive(Debug, Clone)]
pub struct ScheduleRow {
    pub kind: RowKind,
    pub label: String,
    pub id: Option<String>,
    pub due: NaiveDate,
    pub pay_key: String,
    pub payment: f64,
    pub principal: f64,
    pub interest: f64,
    pub balance: f64,
    pub recast: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Scheduled,
    Extra,
}

#[derive(Debug, Clone)]
pub struct ScheduleBuilt {
    pub rows: Vec<ScheduleRow>,
    /// Current monthly payment (after any recasts).
    pub payment: f64,
    pub total_interest: f64,
}

#[derive(Debug, Clone)]
struct Event {
    kind: RowKind,
    n: Option<i32>,
    id: Option<String>,
    amount: Option<f64>,
    due: NaiveDate,
    order: u8,
    recast: bool,
    /// When false, extra is listed but does not change balance or monthly payment.
    applied: bool,
}

/// Extra payment to merge into the amortization schedule.
#[derive(Debug, Clone)]
pub struct ExtraInput {
    pub id: String,
    pub date: NaiveDate,
    pub amount: f64,
    pub recast: bool,
    /// When false, the row is shown but balance / payment / interest are unchanged.
    pub applied: bool,
}

pub fn build_schedule(
    principal: f64,
    annual_rate: f64,
    years: i32,
    start: NaiveDate,
    extras: &[ExtraInput],
) -> ScheduleBuilt {
    let n = years * 12;
    let r = annual_rate / 100.0 / 12.0;
    let mut payment = amort_payment(principal, r, n);

    let mut events: Vec<Event> = Vec::with_capacity(n as usize + extras.len());

    for i in 1..=n {
        let months = i - 1;
        let due = add_months(start, months);
        events.push(Event {
            kind: RowKind::Scheduled,
            n: Some(i),
            id: None,
            amount: None,
            due,
            order: 0,
            recast: false,
            applied: true,
        });
    }

    for ex in extras {
        if ex.amount <= 0.0 {
            continue;
        }
        events.push(Event {
            kind: RowKind::Extra,
            n: None,
            id: Some(ex.id.clone()),
            amount: Some(ex.amount),
            due: ex.date,
            order: 1,
            recast: ex.recast,
            applied: ex.applied,
        });
    }

    events.sort_by(|a, b| a.due.cmp(&b.due).then(a.order.cmp(&b.order)));

    let mut balance = principal;
    let mut remaining = n;
    let mut rows = Vec::with_capacity(events.len());

    for ev in events {
        match ev.kind {
            RowKind::Scheduled => {
                let mut interest = 0.0;
                let mut principal_part = 0.0;
                if balance > 0.005 {
                    interest = if r == 0.0 { 0.0 } else { balance * r };
                    principal_part = payment - interest;
                    if principal_part > balance {
                        principal_part = balance;
                    }
                    if principal_part < 0.0 {
                        interest = interest.min(balance);
                        principal_part = 0.0;
                    }
                }
                let actual_payment = principal_part + interest;
                balance = (balance - principal_part).max(0.0);
                if balance < 0.005 {
                    balance = 0.0;
                }
                remaining = (remaining - 1).max(0);

                rows.push(ScheduleRow {
                    kind: RowKind::Scheduled,
                    label: ev.n.map(|v| v.to_string()).unwrap_or_default(),
                    id: None,
                    due: ev.due,
                    pay_key: ev.due.format("%Y-%m-%d").to_string(),
                    payment: actual_payment,
                    principal: principal_part,
                    interest,
                    balance,
                    recast: false,
                });
            }
            RowKind::Extra => {
                let requested = ev.amount.unwrap_or(0.0);
                let mut principal_part = 0.0;
                if ev.applied && balance > 0.005 {
                    principal_part = requested.min(balance);
                    balance = (balance - principal_part).max(0.0);
                    if balance < 0.005 {
                        balance = 0.0;
                    }
                    if ev.recast && remaining > 0 && balance > 0.005 {
                        payment = amort_payment(balance, r, remaining);
                    }
                }
                let id = ev.id.clone().unwrap_or_default();
                let label = if ev.recast {
                    "Recast".to_string()
                } else {
                    "Extra".to_string()
                };
                let shown = if ev.applied { principal_part } else { requested };
                rows.push(ScheduleRow {
                    kind: RowKind::Extra,
                    label,
                    id: Some(id.clone()),
                    due: ev.due,
                    pay_key: format!("extra:{id}"),
                    payment: shown,
                    principal: shown,
                    interest: 0.0,
                    balance,
                    recast: ev.recast,
                });
            }
        }
    }

    let total_interest = rows.iter().map(|r| r.interest).sum();
    ScheduleBuilt {
        rows,
        payment,
        total_interest,
    }
}

fn amort_payment(balance: f64, r: f64, periods: i32) -> f64 {
    if periods <= 0 || balance <= 0.005 {
        return 0.0;
    }
    if r == 0.0 {
        balance / f64::from(periods)
    } else {
        let pow = (1.0 + r).powi(periods);
        (balance * r * pow) / (pow - 1.0)
    }
}

pub fn payment_status(due: NaiveDate, paid: bool, today: NaiveDate) -> &'static str {
    if paid {
        return "paid";
    }
    if due < today {
        return "missed";
    }
    if due.year() == today.year() && due.month() == today.month() {
        return "due";
    }
    "future"
}

fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let mut year = date.year();
    let mut month = date.month() as i32 + months;
    while month > 12 {
        month -= 12;
        year += 1;
    }
    while month < 1 {
        month += 12;
        year -= 1;
    }
    let day = date.day().min(days_in_month(year, month as u32));
    NaiveDate::from_ymd_opt(year, month as u32, day).expect("valid date")
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if NaiveDate::from_ymd_opt(year, 2, 29).is_some() {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_payment_count() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let built = build_schedule(400_000.0, 6.5, 30, start, &[]);
        assert_eq!(built.rows.len(), 360);
        assert!((built.payment - 2528.27).abs() < 1.0);
    }

    fn applied_extra(
        id: &str,
        date: NaiveDate,
        amount: f64,
        recast: bool,
    ) -> ExtraInput {
        ExtraInput {
            id: id.into(),
            date,
            amount,
            recast,
            applied: true,
        }
    }

    #[test]
    fn extra_without_recast_keeps_payment_shortens_term() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let base = build_schedule(400_000.0, 6.5, 30, start, &[]);
        let extras = [applied_extra(
            "e1",
            NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            50_000.0,
            false,
        )];
        let built = build_schedule(400_000.0, 6.5, 30, start, &extras);
        assert!((built.payment - base.payment).abs() < 0.01);

        let last_nonzero = built
            .rows
            .iter()
            .rev()
            .find(|r| r.kind == RowKind::Scheduled && r.payment > 0.005)
            .unwrap();
        let base_last = base
            .rows
            .iter()
            .rev()
            .find(|r| r.kind == RowKind::Scheduled && r.payment > 0.005)
            .unwrap();
        assert!(last_nonzero.due < base_last.due);
    }

    #[test]
    fn extra_with_recast_lowers_future_payments() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let base = build_schedule(400_000.0, 6.5, 30, start, &[]);
        let extras = [applied_extra(
            "e1",
            NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            50_000.0,
            true,
        )];
        let built = build_schedule(400_000.0, 6.5, 30, start, &extras);
        assert!(built.payment < base.payment - 1.0);

        // Full term still has non-zero scheduled payments near the end.
        let late = built
            .rows
            .iter()
            .filter(|r| r.kind == RowKind::Scheduled)
            .nth(350)
            .unwrap();
        assert!(late.payment > 0.005);

        let recast_row = built.rows.iter().find(|r| r.kind == RowKind::Extra).unwrap();
        assert!(recast_row.recast);
        assert_eq!(recast_row.label, "Recast");

        // First scheduled payment (before recast) keeps original amount.
        let first = built
            .rows
            .iter()
            .find(|r| r.kind == RowKind::Scheduled)
            .unwrap();
        assert!((first.payment - base.payment).abs() < 1.0);

        // Payments after the recast use the new lower amount.
        let after = built
            .rows
            .iter()
            .filter(|r| r.kind == RowKind::Scheduled && r.due > extras[0].date)
            .find(|r| r.payment > 0.005)
            .unwrap();
        assert!((after.payment - built.payment).abs() < 1.0);
    }

    #[test]
    fn unpaid_extra_does_not_change_payment_or_interest() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let base = build_schedule(400_000.0, 6.5, 30, start, &[]);
        let extras = [ExtraInput {
            id: "e1".into(),
            date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            amount: 50_000.0,
            recast: true,
            applied: false,
        }];
        let built = build_schedule(400_000.0, 6.5, 30, start, &extras);
        assert!((built.payment - base.payment).abs() < 0.01);
        assert!((built.total_interest - base.total_interest).abs() < 0.01);

        let row = built.rows.iter().find(|r| r.kind == RowKind::Extra).unwrap();
        assert!((row.payment - 50_000.0).abs() < 0.01);
        // Balance unchanged: matches prior scheduled payment balance.
        let sept = built
            .rows
            .iter()
            .find(|r| {
                r.kind == RowKind::Scheduled
                    && r.due == NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()
            })
            .unwrap();
        assert!((row.balance - sept.balance).abs() < 0.01);
    }
}
