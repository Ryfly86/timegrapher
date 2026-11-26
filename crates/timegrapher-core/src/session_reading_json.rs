//! How a session reading is written: its fields in order, with the beat
//! rate heard after its measurement.

use super::{Cycle, Measurement, Position, Reading, Reference, ShapeSummary};
use serde::Serialize;

impl Serialize for Reading {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        /// The reading as written: its fields in order, with the beat
        /// rate heard after its measurement.
        #[derive(Serialize)]
        struct Written<'a> {
            label: &'a String,
            position: &'a Position,
            wind_h: &'a Option<f64>,
            date: &'a Option<String>,
            notes: &'a Option<String>,
            duration_s: &'a f64,
            bph: &'a u32,
            bph_set: &'a Option<u32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            bph_set_from: Option<crate::set_rate::BphFrom>,
            measurement: &'a Measurement,
            #[serde(flatten)]
            measured: Option<crate::timing::MeasuredRate>,
            cycles: &'a Vec<Cycle>,
            shape: &'a Option<ShapeSummary>,
            reference: &'a Option<Reference>,
        }
        // Every field named, so a new one cannot be left unwritten.
        let Reading {
            label,
            position,
            wind_h,
            date,
            notes,
            duration_s,
            bph,
            bph_set,
            bph_set_from,
            measurement,
            cycles,
            shape,
            reference,
        } = self;
        Written {
            label,
            position,
            wind_h,
            date,
            notes,
            duration_s,
            bph,
            bph_set,
            bph_set_from: bph_set.and(*bph_set_from),
            measurement,
            measured: self.measured(),
            cycles,
            shape,
            reference,
        }
        .serialize(s)
    }
}
