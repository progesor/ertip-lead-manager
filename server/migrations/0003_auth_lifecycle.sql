CREATE TABLE auth_one_time_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
    created_by_user_id TEXT REFERENCES app_users(id) ON DELETE SET NULL,
    purpose TEXT NOT NULL CHECK (purpose IN ('PROVISION', 'RESET')),
    token_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    CHECK (expires_at > created_at)
);

CREATE INDEX idx_auth_one_time_tokens_user_active
    ON auth_one_time_tokens(user_id, purpose, expires_at DESC)
    WHERE used_at IS NULL AND revoked_at IS NULL;

CREATE INDEX idx_auth_one_time_tokens_expiry
    ON auth_one_time_tokens(expires_at)
    WHERE used_at IS NULL AND revoked_at IS NULL;

CREATE TABLE auth_security_events (
    id TEXT PRIMARY KEY,
    target_user_id TEXT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
    actor_user_id TEXT REFERENCES app_users(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'PROVISION_TOKEN_CREATED',
            'RESET_TOKEN_CREATED',
            'CREDENTIAL_ACTIVATED',
            'PASSWORD_CHANGED'
        )
    ),
    occurred_at TIMESTAMPTZ NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_auth_security_events_target_occurred
    ON auth_security_events(target_user_id, occurred_at DESC);

CREATE INDEX idx_auth_security_events_actor_occurred
    ON auth_security_events(actor_user_id, occurred_at DESC);
