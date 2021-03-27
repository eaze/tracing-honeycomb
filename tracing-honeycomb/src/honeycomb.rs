use chrono::{DateTime, Utc};
use eaze_tracing_distributed as tracing_distributed;

use crate::visitor::{event_to_values, span_to_values, HoneycombVisitor};
use libhoney::FieldHolder;
use std::collections::HashMap;
use tracing_distributed::{Event, Span, Telemetry};

#[cfg(feature = "use_parking_lot")]
use parking_lot::Mutex;
#[cfg(not(feature = "use_parking_lot"))]
use std::sync::Mutex;

use crate::{SpanId, TraceId};

/// Telemetry capability that publishes events and spans to Honeycomb.io.
#[derive(Debug)]
pub struct HoneycombTelemetry {
    honeycomb_client: Mutex<libhoney::Client<libhoney::transmission::Transmission>>,
    sample_rate: Option<u32>,
}

impl HoneycombTelemetry {
    pub(crate) fn new(cfg: libhoney::Config, sample_rate: Option<u32>) -> Self {
        let honeycomb_client = libhoney::init(cfg);

        // Handle the libhoney response channel by consuming and ignoring messages. This prevents a
        // deadlock because the responses() channel is bounded and gains an item for every event
        // emitted.
        let responses = honeycomb_client.responses();
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
        let honeycomb_client = Mutex::new(honeycomb_client);

        HoneycombTelemetry {
            honeycomb_client,
            sample_rate,
        }
    }

    fn report_data(&self, data: HashMap<String, libhoney::Value>, timestamp: DateTime<Utc>) {
        // succeed or die. failure is unrecoverable (mutex poisoned)
        #[cfg(not(feature = "use_parking_lot"))]
        let mut client = self.honeycomb_client.lock().unwrap();
        #[cfg(feature = "use_parking_lot")]
        let mut client = self.honeycomb_client.lock();

        let mut ev = client.new_event();
        ev.add(data);
        ev.set_timestamp(timestamp);

        let res = ev.send(&mut client);
        if let Err(err) = res {
            // unable to report telemetry (buffer full) so log msg to stderr
            // TODO: figure out strategy for handling this (eg report data loss event)
            eprintln!("error sending event to honeycomb, {:?}", err);
        }
    }

    fn should_report(&self, trace_id: &TraceId) -> bool {
        if let Some(sample_rate) = self.sample_rate {
            crate::deterministic_sampler::sample(sample_rate, trace_id)
        } else {
            true
        }
    }
}

impl Telemetry for HoneycombTelemetry {
    type Visitor = HoneycombVisitor;
    type TraceId = TraceId;
    type SpanId = SpanId;

    fn mk_visitor(&self) -> Self::Visitor {
        Default::default()
    }

    fn report_span(&self, span: Span<Self::Visitor, Self::SpanId, Self::TraceId>) {
        if self.should_report(&span.trace_id) {
            let (data, timestamp) = span_to_values(span);
            self.report_data(data, timestamp);
        }
    }

    fn report_event(&self, event: Event<Self::Visitor, Self::SpanId, Self::TraceId>) {
        if self.should_report(&event.trace_id) {
            let (data, timestamp) = event_to_values(event);
            self.report_data(data, timestamp);
        }
    }
}
