CREATE TABLE IF NOT EXISTS payment_notes (
    profile_id TEXT NOT NULL,
    pay_key TEXT NOT NULL,
    note TEXT NOT NULL,
    PRIMARY KEY (profile_id, pay_key),
    FOREIGN KEY (profile_id) REFERENCES profiles(id) ON DELETE CASCADE
);
