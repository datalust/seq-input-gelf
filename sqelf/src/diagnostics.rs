use chrono::{DateTime, Utc};
use std::fmt::Display;

#[derive(Serialize)]
struct DiagnosticEvent<'a> {
    #[serde(rename = "@t")]
    timestamp: DateTime<Utc>,

    #[serde(rename = "@l")]
    level: &'static str,

    #[serde(rename = "@mt")]
    message_template: &'static str,

    #[serde(rename = "@x")]
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

impl<'a> DiagnosticEvent<'a> {
    pub fn new(
        level: &'static str,
        error: Option<&'a str>,
        message_template: &'static str,
    ) -> DiagnosticEvent<'a> {
        DiagnosticEvent {
            timestamp: Utc::now(),
            message_template,
            level,
            error,
        }
    }
}

pub fn emit(message_template: &'static str) {
    let evt = DiagnosticEvent::new("DEBUG", None, &message_template);
    let json = serde_json::to_string(&evt).expect("infallible JSON");
    eprintln!("{}", json);
}

pub fn emit_err(error: &impl Display, message_template: &'static str) {
    let err_str = format!("{}", error);
    let evt = DiagnosticEvent::new("ERROR", Some(&err_str), &message_template);
    let json = serde_json::to_string(&evt).expect("infallible JSON");
    eprintln!("{}", json);
}

/// For use with `map_err`
pub(crate) fn emit_abort<TInner>(message_template: &'static str) -> impl Fn(TInner) -> ()
where
    TInner: Display,
{
    move |err| {
        emit_err(&err, message_template);
    }
}

/// For use with `or_else`
pub(crate) fn emit_continue<TInner, TOuter>(
    message_template: &'static str,
) -> impl Fn(TInner) -> Result<(), TOuter>
where
    TInner: Display,
{
    emit_continue_with(message_template, || ())
}

/// For use with `or_else`
pub(crate) fn emit_continue_with<TInner, TOk, TOuter>(
    message_template: &'static str,
    ok: impl Fn() -> TOk,
) -> impl Fn(TInner) -> Result<TOk, TOuter>
where
    TInner: Display,
{
    move |err| {
        emit_err(&err, message_template);

        Ok(ok())
    }
}
