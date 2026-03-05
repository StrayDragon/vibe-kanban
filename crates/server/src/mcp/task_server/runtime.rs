use super::*;

pub(super) fn idempotency_in_progress_ttl() -> Option<chrono::Duration> {
    let raw = match std::env::var(IDEMPOTENCY_IN_PROGRESS_TTL_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => {
            return Some(chrono::Duration::seconds(
                DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS,
            ));
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Failed to read {IDEMPOTENCY_IN_PROGRESS_TTL_ENV}; using default"
            );
            return Some(chrono::Duration::seconds(
                DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS,
            ));
        }
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        tracing::warn!("{IDEMPOTENCY_IN_PROGRESS_TTL_ENV} is set but empty; using default");
        return Some(chrono::Duration::seconds(
            DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS,
        ));
    }

    match trimmed.parse::<i64>() {
        Ok(value) if value <= 0 => None,
        Ok(value) => Some(chrono::Duration::seconds(value)),
        Err(err) => {
            tracing::warn!(
                value = trimmed,
                error = %err,
                "Invalid {IDEMPOTENCY_IN_PROGRESS_TTL_ENV}; using default"
            );
            Some(chrono::Duration::seconds(
                DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS,
            ))
        }
    }
}

pub(super) fn mcp_task_ttl_ms() -> Option<u64> {
    let raw = match std::env::var(MCP_TASK_TTL_MS_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Some(DEFAULT_MCP_TASK_TTL_MS),
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Failed to read {MCP_TASK_TTL_MS_ENV}; using default"
            );
            return Some(DEFAULT_MCP_TASK_TTL_MS);
        }
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        tracing::warn!("{MCP_TASK_TTL_MS_ENV} is set but empty; using default");
        return Some(DEFAULT_MCP_TASK_TTL_MS);
    }

    match trimmed.parse::<i64>() {
        Ok(value) if value <= 0 => None,
        Ok(value) => Some(value as u64),
        Err(err) => {
            tracing::warn!(
                value = trimmed,
                error = %err,
                "Invalid {MCP_TASK_TTL_MS_ENV}; using default"
            );
            Some(DEFAULT_MCP_TASK_TTL_MS)
        }
    }
}

pub(super) fn mcp_task_poll_interval_ms() -> u64 {
    let raw = match std::env::var(MCP_TASK_POLL_INTERVAL_MS_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return DEFAULT_MCP_TASK_POLL_INTERVAL_MS,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Failed to read {MCP_TASK_POLL_INTERVAL_MS_ENV}; using default"
            );
            return DEFAULT_MCP_TASK_POLL_INTERVAL_MS;
        }
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        tracing::warn!("{MCP_TASK_POLL_INTERVAL_MS_ENV} is set but empty; using default");
        return DEFAULT_MCP_TASK_POLL_INTERVAL_MS;
    }

    match trimmed.parse::<u64>() {
        Ok(0) => DEFAULT_MCP_TASK_POLL_INTERVAL_MS,
        Ok(value) => value,
        Err(err) => {
            tracing::warn!(
                value = trimmed,
                error = %err,
                "Invalid {MCP_TASK_POLL_INTERVAL_MS_ENV}; using default"
            );
            DEFAULT_MCP_TASK_POLL_INTERVAL_MS
        }
    }
}

pub(super) fn mcp_task_max_concurrency() -> usize {
    let raw = match std::env::var(MCP_TASK_MAX_CONCURRENCY_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return DEFAULT_MCP_TASK_MAX_CONCURRENCY,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Failed to read {MCP_TASK_MAX_CONCURRENCY_ENV}; using default"
            );
            return DEFAULT_MCP_TASK_MAX_CONCURRENCY;
        }
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        tracing::warn!("{MCP_TASK_MAX_CONCURRENCY_ENV} is set but empty; using default");
        return DEFAULT_MCP_TASK_MAX_CONCURRENCY;
    }

    match trimmed.parse::<usize>() {
        Ok(0) => DEFAULT_MCP_TASK_MAX_CONCURRENCY,
        Ok(value) => value,
        Err(err) => {
            tracing::warn!(
                value = trimmed,
                error = %err,
                "Invalid {MCP_TASK_MAX_CONCURRENCY_ENV}; using default"
            );
            DEFAULT_MCP_TASK_MAX_CONCURRENCY
        }
    }
}
