-- Time-limited invite links and durable profile collaborators.
CREATE TABLE IF NOT EXISTS profile_share_links (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    created_by TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    used_at TEXT,
    revoked_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (profile_id) REFERENCES profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_profile_share_links_profile
    ON profile_share_links(profile_id);

CREATE TABLE IF NOT EXISTS profile_collaborators (
    profile_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'editor',
    invited_by TEXT,
    share_link_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (profile_id, user_id),
    FOREIGN KEY (profile_id) REFERENCES profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (invited_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (share_link_id) REFERENCES profile_share_links(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_profile_collaborators_user
    ON profile_collaborators(user_id);
