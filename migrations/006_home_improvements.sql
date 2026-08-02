CREATE TABLE IF NOT EXISTS home_improvements (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    date TEXT NOT NULL,
    amount REAL NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    detail TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (profile_id) REFERENCES profiles(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_home_improvements_profile_date
  ON home_improvements(profile_id, date);
