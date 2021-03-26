use chrono::{DateTime, Utc};
use libhoney::FieldHolder;
use std::collections::HashMap;

#[cfg(feature = "use_parking_lot")]
use parking_lot::Mutex;
#[cfg(not(feature = "use_parking_lot"))]
use std::sync::Mutex;

/// Reports data to some backend
pub trait Reporter {
    /// Reports data to the backend
    fn report_data(&self, data: HashMap<String, libhoney::Value>, timestamp: DateTime<Utc>);
}

/// Reporter that sends events and spans to a [`libhoney::Client`]
pub type LibhoneyReporter = Mutex<libhoney::Client<libhoney::transmission::Transmission>>;
impl Reporter for LibhoneyReporter {
    fn report_data(&self, data: HashMap<String, libhoney::Value>, timestamp: DateTime<Utc>) {
        // succeed or die. failure is unrecoverable (mutex poisoned)
        #[cfg(not(feature = "use_parking_lot"))]
        let mut reporter = self.lock().unwrap();
        #[cfg(feature = "use_parking_lot")]
        let mut reporter = self.lock();

        let mut ev = reporter.new_event();
        ev.add(data);
        ev.set_timestamp(timestamp);
        let res = ev.send(&mut reporter);
        if let Err(err) = res {
            // unable to report telemetry (buffer full) so log msg to stderr
            // TODO: figure out strategy for handling this (eg report data loss event)
            eprintln!("error sending event to honeycomb, {:?}", err);
        }
    }
}

/// Reporter that sends events and spans to stdout
#[derive(Debug, Clone, Copy)]
pub struct StdoutReporter;
impl Reporter for StdoutReporter {
    fn report_data(&self, data: HashMap<String, libhoney::Value>, _timestamp: DateTime<Utc>) {
        if let Ok(data) = serde_json::to_string(&data) {
            println!("{}", data);
        }
    }
}
