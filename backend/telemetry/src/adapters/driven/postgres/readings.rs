use super::{to_millis, to_timestamp};
use crate::adapters::metrics;
use crate::kernel::ports::ReadingStore;
use crate::types::query::Bucket;
use crate::types::reading::{BuildingTotal, Reading};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::time::Instant;

pub struct PgReadings {
    pool: PgPool,
}

impl PgReadings {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn construct(&self, row: &sqlx::postgres::PgRow) -> Reading {
        let ts: DateTime<Utc> = row.get("ts");
        Reading {
            building_id: row.get("building_id"),
            room_id: row.get("room_id"),
            metric: row.get("metric"),
            ts_ms: to_millis(ts),
            value: row.get("value"),
        }
    }
}

#[async_trait]
impl ReadingStore for PgReadings {
    async fn insert(&self, readings: &[Reading]) -> anyhow::Result<()> {
        if readings.is_empty() {
            return Ok(());
        }

        let started = Instant::now();
        let mut builder = sqlx::QueryBuilder::new(
            "insert into readings (building_id, room_id, metric, ts, value) ",
        );
        builder.push_values(readings, |mut row, reading| {
            row.push_bind(&reading.building_id)
                .push_bind(&reading.room_id)
                .push_bind(&reading.metric)
                .push_bind(to_timestamp(reading.ts_ms))
                .push_bind(reading.value);
        });
        builder.push(" on conflict do nothing");
        let inserted = builder.build().execute(&self.pool).await;

        let elapsed = started.elapsed();
        for reading in readings {
            metrics::record_persist_duration(&reading.metric, elapsed);
            if inserted.is_err() {
                metrics::record_persist_failure(&reading.metric);
            }
        }
        inserted?;
        Ok(())
    }

    async fn latest(
        &self,
        building_id: &str,
        metric: &str,
        room_id: &str,
    ) -> anyhow::Result<Option<Reading>> {
        let row = sqlx::query(
            "select building_id, room_id, metric, ts, value from readings
             where building_id = $1 and metric = $2 and room_id = $3
             order by ts desc limit 1",
        )
        .bind(building_id)
        .bind(metric)
        .bind(room_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| self.construct(&row)))
    }

    async fn latest_per_room(
        &self,
        building_id: &str,
        metric: &str,
    ) -> anyhow::Result<Vec<Reading>> {
        let rows = sqlx::query(
            "select distinct on (room_id)
                 building_id, room_id, metric, ts, value
             from readings
             where building_id = $1 and metric = $2
             order by room_id, ts desc",
        )
        .bind(building_id)
        .bind(metric)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|row| self.construct(row)).collect())
    }

    async fn building_total(
        &self,
        building_id: &str,
        metric: &str,
    ) -> anyhow::Result<Option<BuildingTotal>> {
        // distinct on room: a report delivered twice must not double the total.
        let row = sqlx::query(
            "with newest as (
                 select max(ts) as ts from readings where building_id = $1 and metric = $2
             ),
             rooms as (
                 select distinct on (room_id) r.ts, r.value
                 from readings r, newest
                 where r.building_id = $1 and r.metric = $2 and r.ts = newest.ts
                 order by room_id
             )
             select min(ts) as ts, sum(value) as total from rooms",
        )
        .bind(building_id)
        .bind(metric)
        .fetch_one(&self.pool)
        .await?;
        let ts: Option<DateTime<Utc>> = row.get("ts");
        Ok(ts.map(|ts| BuildingTotal {
            ts_ms: to_millis(ts),
            value: row.get::<Option<f64>, _>("total").unwrap_or_default(),
        }))
    }

    async fn series(
        &self,
        building_id: &str,
        metric: &str,
        room_id: Option<&str>,
        window: (i64, i64),
        bucket: &str,
        agg: &str,
    ) -> anyhow::Result<Vec<Bucket>> {
        if !["avg", "sum", "min", "max"].contains(&agg) {
            anyhow::bail!("unsupported aggregate: {agg}");
        }
        let rollup = bucket != "1 hour";
        let value = match (rollup, agg) {
            // An average of hourly averages would weigh a 5-reading hour like a 120-reading one.
            (true, "avg") => "sum(sum) / sum(samples)".to_owned(),
            (true, _) => format!("{agg}({agg})"),
            (false, _) => format!("{agg}(value)"),
        };
        let source = if rollup {
            "readings_hourly"
        } else {
            "readings"
        };
        let column = if rollup { "bucket" } else { "ts" };

        let sql = format!(
            "select time_bucket($1::interval, {column}) as slot, {value} as value
             from {source}
             where building_id = $2 and metric = $3
               and {column} >= $4 and {column} <= $5
               and ($6::text is null or room_id = $6)
             group by slot order by slot asc"
        );

        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(bucket)
            .bind(building_id)
            .bind(metric)
            .bind(to_timestamp(window.0))
            .bind(to_timestamp(window.1))
            .bind(room_id)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .iter()
            .map(|row| Bucket {
                ts_ms: to_millis(row.get::<DateTime<Utc>, _>("slot")),
                value: row.get::<f64, _>("value"),
            })
            .collect())
    }
}
