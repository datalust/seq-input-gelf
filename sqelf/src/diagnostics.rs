use chrono::{DateTime, Utc};
use std::fmt::Display;

#[derive(Serialize)]
struct DiagnosticEvent<'a> {
    #[serde(rename="@t")]
    timestamp: DateTime<Utc>,

    #[serde(rename="@l")]
    level: &'static str,

    #[serde(rename="@mt")]
    message_template: &'static str,

    #[serde(rename="@x")]
    error: Option<&'a str>,
}

impl<'a> DiagnosticEvent<'a> {
    pub fn new(level: &'static str, error: Option<&'a str>, message_template: &'static str) -> DiagnosticEvent<'a> {
        DiagnosticEvent {
            timestamp: Utc::now(),
            message_template,
            level,
            error,
        }
    }
}

pub fn emit_err(error: &impl Display, message_template: &'static str) {
    let err_str = format!("{}", error);
    let evt = DiagnosticEvent::new("ERROR", Some(&err_str), &message_template);
    let json = serde_json::to_string(&evt).expect("infallible JSON");
    eprintln!("{}", json);
}
