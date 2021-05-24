#![deny(
    warnings,
    missing_debug_implementations,
    missing_copy_implementations,
    missing_docs
)]

//! This crate provides:
//! - A tracing layer, `TelemetryLayer`, that can be used to publish trace data to honeycomb.io
//! - Utilities for implementing distributed tracing against the honeycomb.io backend
//!
//! As a tracing layer, `TelemetryLayer` can be composed with other layers to provide stdout logging, filtering, etc.

mod honeycomb;
mod reporter;
mod span_id;
mod trace_id;
mod visitor;

pub use honeycomb::HoneycombTelemetry;
pub use reporter::{LibhoneyReporter, Reporter, StdoutReporter};
pub use span_id::SpanId;
pub use trace_id::TraceId;
#[doc(no_inline)]
pub use tracing_distributed::{TelemetryLayer, TraceCtxError};
pub use visitor::HoneycombVisitor;

pub(crate) mod deterministic_sampler;

#[cfg(feature = "use_parking_lot")]
use parking_lot::Mutex;
#[cfg(not(feature = "use_parking_lot"))]
use std::sync::Mutex;

/// Register the current span as the local root of a distributed trace.
///
/// Specialized to the honeycomb.io-specific SpanId and TraceId provided by this crate.
pub fn register_dist_tracing_root(
    trace_id: TraceId,
    remote_parent_span: Option<SpanId>,
) -> Result<(), TraceCtxError> {
    tracing_distributed::register_dist_tracing_root(trace_id, remote_parent_span)
}

/// Retrieve the distributed trace context associated with the current span.
///
/// Returns the `TraceId`, if any, that the current span is associated with along with
/// the `SpanId` belonging to the current span.
///
/// Specialized to the honeycomb.io-specific SpanId and TraceId provided by this crate.
pub fn current_dist_trace_ctx() -> Result<(TraceId, SpanId), TraceCtxError> {
    tracing_distributed::current_dist_trace_ctx()
}

/// Construct a TelemetryLayer that does not publish telemetry to any backend.
///
/// Specialized to the honeycomb.io-specific SpanId and TraceId provided by this crate.
pub fn new_blackhole_telemetry_layer(
) -> TelemetryLayer<tracing_distributed::BlackholeTelemetry<SpanId, TraceId>, SpanId, TraceId> {
    TelemetryLayer::new(
        "honeycomb_blackhole_tracing_layer",
        tracing_distributed::BlackholeTelemetry::default(),
        move |tracing_id| SpanId { tracing_id },
    )
}

/// Construct a TelemetryLayer that publishes telemetry to honeycomb.io using the provided honeycomb config.
///
/// Specialized to the honeycomb.io-specific SpanId and TraceId provided by this crate.
pub fn new_honeycomb_telemetry_layer(
    service_name: &'static str,
    honeycomb_config: libhoney::Config,
) -> TelemetryLayer<HoneycombTelemetry<LibhoneyReporter>, SpanId, TraceId> {
    let reporter = libhoney::init(honeycomb_config);
    // publishing requires &mut so just mutex-wrap it
    // FIXME: may not be performant, investigate options (eg mpsc)
    let reporter = Mutex::new(reporter);

    TelemetryLayer::new(
        service_name,
        HoneycombTelemetry::new(reporter, None),
        move |tracing_id| SpanId { tracing_id },
    )
}

/// Construct a TelemetryLayer that publishes telemetry to honeycomb.io using the
/// provided honeycomb config, and sample rate.
///
/// This function differs from `new_honeycomb_telemetry_layer` and the `sample_rate`
/// on the `libhoney::Config` there in an important way. `libhoney` samples `Event`
/// data, which is individual spans on each trace. This means that using the
/// sampling logic in libhoney may result in missing event data or incomplete
/// traces. Calling this function provides trace-level sampling, meaning sampling
/// decisions are based on a modulo of the traceID, and events in a single trace
/// will not be sampled differently. If the trace is sampled, then all spans
/// under it will be sent to honeycomb. If a trace is not sampled, no spans or
/// events under it will be sent. When using this trace-level sampling, the
/// `sample_rate` parameter on the `libhoney::Config` should be set to 1, which
/// is the default.
///
/// Specialized to the honeycomb.io-specific SpanId and TraceId provided by this crate.
pub fn new_honeycomb_telemetry_layer_with_trace_sampling(
    service_name: &'static str,
    honeycomb_config: libhoney::Config,
    sample_rate: u32,
) -> TelemetryLayer<HoneycombTelemetry<LibhoneyReporter>, SpanId, TraceId> {
    let reporter = libhoney::init(honeycomb_config);
    // publishing requires &mut so just mutex-wrap it
    // FIXME: may not be performant, investigate options (eg mpsc)
    let reporter = Mutex::new(reporter);

    TelemetryLayer::new(
        service_name,
        HoneycombTelemetry::new(reporter, Some(sample_rate)),
        move |tracing_id| SpanId { tracing_id },
    )
}

/// Builds Honeycomb Telemetry with custom configuration values.
///
/// Methods can be chained in order to set the configuration values. The
/// TelemetryLayer is constructed by calling [`build`].
///
/// New instances of `Builder` are obtained via [`Builder::new_libhoney`]
/// or [`Builder::new_stdout`].
///
/// [`Builder::new_stdout`] is useful when instrumenting e.g. AWS Lambda functions.
/// See more at [AWS Lambda Instrumentation]. For almost all other use cases you are probably
/// looking for [`Builder::new_libhoney`].
///
/// [`build`]: method@Self::build
/// [`Builder::new_stdout`]: method@Builder::<StdoutReporter>::new_stdout
/// [`Builder::new_libhoney`]: method@Builder::<LibhoneyReporter>::new_libhoney
/// [AWS Lambda Instrumentation]: https://docs.honeycomb.io/getting-data-in/integrations/aws/aws-lambda/
#[derive(Debug)]
pub struct Builder<R> {
    reporter: R,
    sample_rate: Option<u32>,
    service_name: &'static str,
}

impl Builder<StdoutReporter> {
    /// Returns a new `Builder` that reports data to stdout
    pub fn new_stdout(service_name: &'static str) -> Self {
        Self {
            reporter: StdoutReporter,
            sample_rate: None,
            service_name,
        }
    }
}

impl Builder<LibhoneyReporter> {
    /// Returns a new `Builder` that reports data to a [`libhoney::Client`]
    pub fn new_libhoney(service_name: &'static str, config: libhoney::Config) -> Self {
        let reporter = libhoney::init(config);

        // Handle the libhoney response channel by consuming and ignoring messages. This prevents a
        // deadlock because the responses() channel is bounded and gains an item for every event
        // emitted.
        let responses = reporter.responses();
        std::thread::spawn(move || {
            loop {
                if responses.recv().is_err() {
                    // If we receive an error, the channel is empty & disconnected. No need to keep
                    // this thread around.
                    break;
                }
            }
        });

        // publishing requires &mut so just mutex-wrap it
        // FIXME: may not be performant, investigate options (eg mpsc)
        let reporter = Mutex::new(reporter);

        Self {
            reporter,
            sample_rate: None,
            service_name,
        }
    }
}

impl<R: Reporter> Builder<R> {
    /// Enables sampling for the telemetry layer.
    ///
    /// The `sample_rate` on the `libhoney::Config` is different from this in an important way.
    /// `libhoney` samples `Event` data, which is individual spans on each trace.
    /// This means that using the sampling logic in libhoney may result in missing
    /// event data or incomplete traces.
    /// Calling this function provides trace-level sampling, meaning sampling
    /// decisions are based on a modulo of the traceID, and events in a single trace
    /// will not be sampled differently. If the trace is sampled, then all spans
    /// under it will be sent to honeycomb. If a trace is not sampled, no spans or
    /// events under it will be sent. When using this trace-level sampling,
    /// when using a [`LibhoneyReporter`] the `sample_rate` parameter on the
    /// [`libhoney::Config`] should be set to 1, which is the default.
    pub fn with_trace_sampling(mut self, sample_rate: u32) -> Self {
        self.sample_rate.replace(sample_rate);
        self
    }

    /// Constructs the configured `TelemetryLayer`
    pub fn build(self) -> TelemetryLayer<HoneycombTelemetry<R>, SpanId, TraceId> {
        TelemetryLayer::new(
            self.service_name,
            HoneycombTelemetry::new(self.reporter, self.sample_rate),
            move |tracing_id| SpanId { tracing_id },
        )
    }
}
