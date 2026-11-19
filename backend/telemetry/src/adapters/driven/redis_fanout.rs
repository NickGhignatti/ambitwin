use crate::adapters::metrics;
use crate::kernel::ports::Fanout;
use crate::types::event::TelemetryEvent;
use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::MultiplexedConnection;
use serde_json::Value;
use telemetry_schema::{RAW_CHANNEL, TelemetryEnvelope, TelemetryReading};

pub struct RedisFanout {
    connection: MultiplexedConnection,
}

impl RedisFanout {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(url)?;
        let connection = client.get_multiplexed_async_connection().await?;
        Ok(Self { connection })
    }

    async fn publish(&self, channel: &str, payload: &Value) -> &'static str {
        // Cloned per publish, not locked: MultiplexedConnection shares one
        // socket and pipelines concurrent commands. A Mutex here would make
        // every ingest wait a full round trip for the one before it.
        let mut connection = self.connection.clone();
        let published: redis::RedisResult<()> =
            connection.publish(channel, payload.to_string()).await;
        match published {
            Ok(()) => "ok",
            Err(error) => {
                log::error!("failed to publish on {channel}: {error}");
                "error"
            }
        }
    }
}

fn telemetry_json(event: &TelemetryEvent) -> Value {
    let reading = TelemetryReading {
        metric: event.metric.clone(),
        room_id: event.room_id.clone(),
        ts_ms: event.ts_ms,
        value: event.value,
    };
    serde_json::to_value(reading).expect("a reading always serialises")
}

fn telemetry_batch_json(events: &[TelemetryEvent]) -> Value {
    let envelope = TelemetryEnvelope {
        building_id: events
            .first()
            .map(|event| event.building_id.clone())
            .unwrap_or_default(),
        ingested_at_ms: events
            .first()
            .map(|event| event.ingested_at_ms)
            .unwrap_or(0),
        readings: events.iter().map(telemetry_json).collect(),
    };
    serde_json::to_value(envelope).expect("an envelope always serialises")
}

#[async_trait]
impl Fanout for RedisFanout {
    async fn publish_telemetry(&self, events: &[TelemetryEvent]) {
        if events.is_empty() {
            return;
        }
        let outcome = self
            .publish(RAW_CHANNEL, &telemetry_batch_json(events))
            .await;
        metrics::record_telemetry_published(outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(metric: &str, value: f64) -> TelemetryEvent {
        TelemetryEvent {
            metric: metric.to_owned(),
            building_id: "b1".to_owned(),
            room_id: "r1".to_owned(),
            ts_ms: 1_699_999_000_000,
            value,
            ingested_at_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn a_batch_is_one_envelope_carrying_every_reading_of_the_tick() {
        let body = telemetry_batch_json(&[event("temperature", 21.5), event("co2", 612.0)]);

        assert_eq!(body["buildingId"], "b1");
        assert_eq!(body["ingestedAt"], 1_700_000_000_000i64);
        assert!(body.get("type").is_none(), "`type` belongs only to a reading");
        let reading = &body["readings"][1];
        assert_eq!(reading["type"], "co2");
        assert_eq!(reading["roomId"], "r1");
        assert_eq!(reading["value"], 612.0);
        assert!(reading.get("buildingId").is_none(), "the envelope owns the building");
    }

    const WIRE: &str = include_str!("../../../../../schemas/fixtures/telemetry-envelope.json");

    fn events_of(tick: &Value) -> Vec<TelemetryEvent> {
        tick["readings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| TelemetryEvent {
                metric: r["type"].as_str().unwrap().to_owned(),
                building_id: tick["buildingId"].as_str().unwrap().to_owned(),
                room_id: r["roomId"].as_str().unwrap().to_owned(),
                ts_ms: r["timestamp"].as_i64().unwrap(),
                value: r["value"].as_f64().unwrap(),
                ingested_at_ms: tick["ingestedAt"].as_i64().unwrap(),
            })
            .collect()
    }

    #[test]
    fn every_tick_is_published_as_the_fixture_byte_for_byte() {
        let wire: Value = serde_json::from_str(WIRE).unwrap();
        for case in wire["cases"].as_array().unwrap() {
            assert_eq!(
                telemetry_batch_json(&events_of(&case["body"])),
                case["body"],
                "{}",
                case["name"]
            );
        }
    }
}
