use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct Message<TString, TMessage = TString> {
    // GELF built-ins
    pub(super) version: Option<TString>,
    pub(super) host: Option<TString>,
    pub(super) short_message: TMessage,
    pub(super) full_message: Option<TMessage>,
    pub(super) timestamp: Option<f64>,
    pub(super) level: Option<u8>,

    // Deprecated built-ins, still may be present
    pub(super) facility: Option<TMessage>,
    pub(super) line: Option<u32>,
    pub(super) file: Option<TMessage>,

    // Everything else
    #[serde(flatten)]
    pub(super) additional: Option<Value>,
}
