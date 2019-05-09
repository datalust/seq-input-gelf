use crate::error::{err_msg, Error};
use chrono::{DateTime, Utc};
use std::{
    collections::HashMap,
    fmt::Display,
    ops::Drop,
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

/**
Diagnostics configuration.
*/
#[derive(Debug, Clone)]
pub struct Config {
    /**
    The interval to sample metrics at.
    */
    pub metrics_interval_ms: u64,
    /**
    The minimum self log level to emit.
    */
    pub min_level: Level,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            metrics_interval_ms: 5 * 1000 * 60,
            min_level: Level::Error,
        }
    }
}

pub(crate) struct Diagnostics {
    metrics: Option<(mpsc::Sender<()>, thread::JoinHandle<()>)>,
}

impl Diagnostics {
    pub fn stop_metrics(&mut self) -> Result<(), Error> {
        if let Some((tx, handle)) = self.metrics.take() {
            tx.send(())?;

            handle
                .join()
                .map_err(|_| err_msg("failed to join diagnostics handle"))?;
        }

        Ok(())
    }
}

impl Drop for Diagnostics {
    fn drop(&mut self) {
        if let Some((tx, _)) = self.metrics.take() {
            let _ = tx.send(());
        }
    }
}

pub(crate) fn init(config: Config) -> Diagnostics {
    MIN_LEVEL.set(config.min_level);

    // Only set up metrics if the minimum level is Debug
    let metrics = if MIN_LEVEL.includes(Level::Debug) {
        let (tx, rx) = mpsc::channel();

        let metrics_timeout = Duration::from_millis(config.metrics_interval_ms);
        let handle = thread::spawn(move || loop {
            match rx.recv_timeout(metrics_timeout) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                _ => {
                    emit_metrics();
                }
            }
        });

        Some((tx, handle))
    } else {
        None
    };

    Diagnostics { metrics }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Error,
}

impl FromStr for Level {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "DEBUG" => Ok(Level::Debug),
            "ERROR" => Ok(Level::Error),
            _ => Err(err_msg("expected `DEBUG` or `ERROR`")),
        }
    }
}

impl Level {
    fn to_usize(self) -> usize {
        match self {
            Level::Debug => 0,
            Level::Error => 1,
        }
    }

    fn from_usize(v: usize) -> Self {
        match v {
            0 => Level::Debug,
            _ => Level::Error,
        }
    }
}

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

    #[serde(flatten)]
    additional: Option<serde_json::Value>,
}

impl<'a> DiagnosticEvent<'a> {
    pub fn new(
        level: &'static str,
        error: Option<&'a str>,
        message_template: &'static str,
        additional: Option<serde_json::Value>,
    ) -> DiagnosticEvent<'a> {
        DiagnosticEvent {
            timestamp: Utc::now(),
            message_template,
            level,
            error,
            additional,
        }
    }
}

pub fn emit(message_template: &'static str) {
    if MIN_LEVEL.includes(Level::Debug) {
        let evt = DiagnosticEvent::new("DEBUG", None, &message_template, None);
        let json = serde_json::to_string(&evt).expect("infallible JSON");
        eprintln!("{}", json);
    }
}

pub fn emit_err(error: &impl Display, message_template: &'static str) {
    if MIN_LEVEL.includes(Level::Error) {
        let err_str = format!("{}", error);
        let evt = DiagnosticEvent::new("ERROR", Some(&err_str), &message_template, None);
        let json = serde_json::to_string(&evt).expect("infallible JSON");
        eprintln!("{}", json);
    }
}

pub fn emit_metrics() {
    if MIN_LEVEL.includes(Level::Debug) {
        #[derive(Serialize)]
        struct EmitMetrics {
            receive: HashMap<&'static str, usize>,
            process: HashMap<&'static str, usize>,
            server: HashMap<&'static str, usize>,
        }

        let mut metrics = EmitMetrics {
            receive: HashMap::new(),
            process: HashMap::new(),
            server: HashMap::new(),
        };

        let receive = METRICS.receive.take();
        let process = METRICS.process.take();
        let server = METRICS.server.take();

        metrics.receive.extend(receive.as_ref().iter().cloned());
        metrics.process.extend(process.as_ref().iter().cloned());
        metrics.server.extend(server.as_ref().iter().cloned());

        let metrics = serde_json::to_value(metrics).expect("infallible JSON");

        let evt = DiagnosticEvent::new(
            "DEBUG",
            None,
            "collected GELF server metrics",
            Some(metrics),
        );
        let json = serde_json::to_string(&evt).expect("infallible JSON");

        eprintln!("{}", json);
    }
}

/// For use with `map_err`
pub(crate) fn emit_abort<TInner>(message_template: &'static str) -> impl Fn(TInner) -> ()
where
    TInner: Display,
{
    emit_abort_with(message_template, || ())
}

/// For use with `map_err`
pub(crate) fn emit_abort_with<TInner, TError>(
    message_template: &'static str,
    err: impl Fn() -> TError,
) -> impl Fn(TInner) -> TError
where
    TInner: Display,
{
    move |e| {
        emit_err(&e, message_template);

        err()
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

pub(crate) struct MinLevel(AtomicUsize);
pub(crate) static MIN_LEVEL: MinLevel = MinLevel(AtomicUsize::new(0));

impl MinLevel {
    fn set(&self, min: Level) {
        MIN_LEVEL.0.store(min.to_usize(), Ordering::Relaxed);
    }

    fn get(&self) -> Level {
        Level::from_usize(MIN_LEVEL.0.load(Ordering::Relaxed))
    }

    pub(crate) fn includes(&self, level: Level) -> bool {
        level.to_usize() >= self.get().to_usize()
    }
}

pub(crate) struct Metrics {
    pub(crate) receive: crate::receive::Metrics,
    pub(crate) process: crate::process::Metrics,
    pub(crate) server: crate::server::Metrics,
    _private: (),
}

pub(crate) static METRICS: Metrics = Metrics {
    receive: crate::receive::Metrics::new(),
    process: crate::process::Metrics::new(),
    server: crate::server::Metrics::new(),
    _private: (),
};

macro_rules! increment {
    ($($metric:tt)*) => {{
        if $crate::diagnostics::MIN_LEVEL.includes($crate::diagnostics::Level::Debug) {
            $crate::diagnostics::METRICS.$($metric)*.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }};
}

macro_rules! metrics {
    ($($metric:ident),*) => {
        pub(crate) struct Metrics {
            $(
                pub(crate) $metric: std::sync::atomic::AtomicUsize,
            )*
            _private: (),
        }

        impl Metrics {
            pub(crate) const fn new() -> Self {
                Metrics {
                    $(
                        $metric: std::sync::atomic::AtomicUsize::new(0),
                    )*
                    _private: (),
                }
            }

            pub(crate) fn take(&self) -> impl AsRef<[(&'static str, usize)]> {
                let fields = [
                    $(
                        (stringify!($metric), self.$metric.swap(0, std::sync::atomic::Ordering::Relaxed)),
                    )*
                ];

                fields
            }
        }
    };
}
