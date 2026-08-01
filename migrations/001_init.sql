-- Homestead mortgage ledger schema
CREATE TABLE IF NOT EXISTS profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    principal REAL,
    rate REAL,
    term_years INTEGER,
    start_date TEXT,
    monthly_payment REAL,
    total_interest REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS paid_keys (
    profile_id TEXT NOT NULL,
    pay_key TEXT NOT NULL,
    PRIMARY KEY (profile_id, pay_key),
    FOREIGN KEY (profile_id) REFERENCES profiles(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS extras (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    date TEXT NOT NULL,
    amount REAL NOT NULL,
    recast INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (profile_id) REFERENCES profiles(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO settings (key, value) VALUES ('active_profile_id', '');
