use crate::error::GovernanceError;
use serde_json::Value;

pub struct WebhookProcessor;

impl WebhookProcessor {
    pub fn process_webhook(payload: &Value) -> Result<WebhookEvent, GovernanceError> {
        let action = payload
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let event_type = match action {
            "opened" | "synchronize" | "reopened" => WebhookEventType::PullRequest,
            "submitted" => WebhookEventType::Review,
            "created" => WebhookEventType::Comment,
            "push" => WebhookEventType::Push,
            _ => WebhookEventType::Unknown,
        };

        Ok(WebhookEvent {
            event_type,
            action: action.to_string(),
            payload: payload.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub enum WebhookEventType {
    PullRequest,
    Review,
    Comment,
    Push,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct WebhookEvent {
    pub event_type: WebhookEventType,
    pub action: String,
    pub payload: Value,
}
