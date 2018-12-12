use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct Message<TString, TMessage = TString> {
    // GELF
    pub(super) version: TString,
    pub(super) host: TString,
    pub(super) short_message: TMessage,
    pub(super) full_message: Option<TMessage>,
    pub(super) timestamp: Option<f64>,
    pub(super) level: Option<u8>,

    // Common Docker parameters
    #[serde(rename = "_container_id")]
    pub(super) container_id: Option<TString>,
    #[serde(rename = "_command")]
    pub(super) command: Option<TString>,
    #[serde(rename = "_container_name")]
    pub(super) container_name: Option<TString>,
    #[serde(rename = "_created")]
    pub(super) created: Option<TString>,
    #[serde(rename = "_image_name")]
    pub(super) image_name: Option<TString>,
    #[serde(rename = "_image_id")]
    pub(super) image_id: Option<TString>,
    #[serde(rename = "_tag")]
    pub(super) tag: Option<TString>,

    // Everything else
    #[serde(flatten)]
    pub(super) additional: Option<Value>,
}
