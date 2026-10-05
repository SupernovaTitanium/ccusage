//! Evidence captured from the same accepted loader entries, never raw message bodies.
use std::cell::RefCell;

use serde_json::{Value, json};

use crate::{LoadedEntry, format_rfc3339_millis, json_float};

thread_local! {
    static RECORDS: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn take() -> Vec<Value> {
    RECORDS.with(|records| std::mem::take(&mut *records.borrow_mut()))
}

pub(super) fn extend(records: Vec<Value>) {
    RECORDS.with(|current| current.borrow_mut().extend(records));
}

pub(super) fn capture(agent: &str, entries: &[LoadedEntry]) {
    RECORDS.with(|records| {
        let mut records = records.borrow_mut();
        for entry in entries {
            let usage = &entry.data.message.usage;
            let (cache_creation_5m, cache_creation_1h, cache_creation_unbucketed) =
                usage.cache_creation_buckets();
            let model = entry.model.as_deref().or(entry.data.message.model.as_deref()).unwrap_or("unknown");
            // Native IDs are preserved when available. Otherwise the accepted timestamp
            // and session form an explicit derived identity (no invented parent relation).
            let identity = json!([agent, entry.project_path.as_ref(), entry.session_id.as_ref(),
                entry.data.request_id, entry.data.message.id, entry.data.timestamp, model]).to_string();
            records.push(json!({
                "schemaVersion": 1, "usageId": identity, "tool": agent,
                "source": "native", "sessionId": entry.session_id.as_ref(),
                "timestamp": format_rfc3339_millis(entry.timestamp), "day": entry.date,
                "rawModel": entry.data.message.model, "model": model,
                "inputTokens": usage.input_tokens, "outputTokens": usage.output_tokens,
                "cacheReadTokens": usage.cache_read_input_tokens,
                "cacheCreationTokens": usage.cache_creation_token_count(),
                "cacheCreation5mTokens": cache_creation_5m,
                "cacheCreation1hTokens": cache_creation_1h,
                "cacheCreationUnbucketedTokens": cache_creation_unbucketed,
                "totalTokens": usage.input_tokens.saturating_add(usage.output_tokens)
                    .saturating_add(usage.cache_read_input_tokens)
                    .saturating_add(usage.cache_creation_token_count())
                    .saturating_add(entry.extra_total_tokens),
                "cost": entry.cost,
                "pricingStatus": if entry.missing_pricing_model.is_some() { "missing" } else { "baked" },
                "attributionStatus": "accepted_loader_entry", "detailLevel": "event"
            }));
        }
    });
}

pub(super) fn bundle(rows: &[super::types::AllRow], mut records: Vec<Value>) -> Value {
    records.retain(|record| rows.iter().any(|row| record["day"] == row.period));
    for row in rows {
        let agents = row
            .agent_breakdowns
            .as_deref()
            .unwrap_or(std::slice::from_ref(row));
        for agent in agents {
            if records
                .iter()
                .any(|record| record["tool"] == agent.agent && record["day"] == row.period)
            {
                continue;
            }
            // Sources without usable event provenance retain explicit aggregate
            // evidence, never fabricated events.
            for model in &agent.model_breakdowns {
                records.push(json!({
                    "schemaVersion": 1,
                    "usageId": json!(["aggregate", row.period, agent.agent, model.model_name]).to_string(),
                    "tool": agent.agent, "source": "native", "sessionId": null,
                    "timestamp": null, "day": row.period, "rawModel": null, "model": model.model_name,
                    "inputTokens": model.input_tokens, "outputTokens": model.output_tokens,
                    "cacheReadTokens": model.cache_read_tokens, "cacheCreationTokens": model.cache_creation_tokens,
                    "cacheCreation5mTokens": model.cache_creation_5m_tokens,
                    "cacheCreation1hTokens": model.cache_creation_1h_tokens,
                    "cacheCreationUnbucketedTokens": model.cache_creation_unbucketed_tokens,
                    "totalTokens": model.input_tokens.saturating_add(model.output_tokens)
                        .saturating_add(model.cache_read_tokens).saturating_add(model.cache_creation_tokens)
                        .saturating_add(model.extra_total_tokens),
                    "cost": json_float(model.cost), "pricingStatus": "baked_aggregate",
                    "attributionStatus": "daily_tool_model_only", "detailLevel": "aggregate_only"
                }));
            }
        }
    }
    json!({"schemaVersion": 1, "records": records})
}
