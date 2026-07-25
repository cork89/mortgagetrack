use chrono::{Datelike, NaiveDate};

#[derive(Debug, Clone)]
pub struct ScheduleRow {
    pub kind: RowKind,
    pub n: Option<i32>,
    pub label: String,
    pub id: Option<String>,
    pub due: NaiveDate,
    pub pay_key: String,
    pub payment: f64,
    pub principal: f64,
    pub interest: f64,
    pub balance: f64,
    pub requested: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Scheduled,
    Extra,
}

#[derive(Debug, Clone)]
pub struct ScheduleBuilt {
    pub rows: Vec<ScheduleRow>,
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
}

pub fn build_schedule(
    principal: f64,
    annual_rate: f64,
    years: i32,
    start: NaiveDate,
    extras: &[(String, NaiveDate, f64)],
) -> ScheduleBuilt {
    let n = years * 12;
    let r = annual_rate / 100.0 / 12.0;
    let payment = if r == 0.0 {
        principal / f64::from(n)
    } else {
        let pow = (1.0 + r).powi(n);
        (principal * r * pow) / (pow - 1.0)
    };

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
        });
    }

    for (id, date, amount) in extras {
        if *amount <= 0.0 {
            continue;
        }
        events.push(Event {
            kind: RowKind::Extra,
            n: None,
            id: Some(id.clone()),
            amount: Some(*amount),
            due: *date,
            order: 1,
        });
    }

    events.sort_by(|a, b| a.due.cmp(&b.due).then(a.order.cmp(&b.order)));

    let mut balance = principal;
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

                rows.push(ScheduleRow {
                    kind: RowKind::Scheduled,
                    n: ev.n,
                    label: ev.n.map(|v| v.to_string()).unwrap_or_default(),
                    id: None,
                    due: ev.due,
                    pay_key: ev.due.format("%Y-%m-%d").to_string(),
                    payment: actual_payment,
                    principal: principal_part,
                    interest,
                    balance,
                    requested: None,
                });
            }
            RowKind::Extra => {
                let requested = ev.amount.unwrap_or(0.0);
                let mut principal_part = 0.0;
                if balance > 0.005 {
                    principal_part = requested.min(balance);
                }
                balance = (balance - principal_part).max(0.0);
                if balance < 0.005 {
                    balance = 0.0;
                }
                let id = ev.id.clone().unwrap_or_default();
                rows.push(ScheduleRow {
                    kind: RowKind::Extra,
                    n: None,
                    label: "Extra".to_string(),
                    id: Some(id.clone()),
                    due: ev.due,
                    pay_key: format!("extra:{id}"),
                    payment: principal_part,
                    principal: principal_part,
                    interest: 0.0,
                    balance,
                    requested: Some(requested),
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
}
