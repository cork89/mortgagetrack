-- Sessions and auth rate limits (replaces Redis / Upstash).
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY NOT NULL,
    data TEXT NOT NULL,
    expiry_date TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS sessions_expiry_date_idx ON sessions (expiry_date);

CREATE TABLE IF NOT EXISTS rate_limits (
    key TEXT PRIMARY KEY NOT NULL,
    count INTEGER NOT NULL,
    expires_at TEXT NOT NULL
);
