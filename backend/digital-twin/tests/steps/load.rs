use std::time::{Duration, Instant};

use axum::http::StatusCode;
use cucumber::{then, when};

use crate::support::fixtures::{admin_of, building_with_n_rooms};
use crate::support::http_client::send;
use crate::support::registration::wait_for_status;
use crate::support::world::{LoadRun, TwinWorld};

const MIN_SAMPLES_FOR_P99: usize = 100;

fn p99(mut samples: Vec<Duration>) -> Duration {
    assert!(
        samples.len() >= MIN_SAMPLES_FOR_P99,
        "p99 over {} samples is just the max, not a percentile — need at least {MIN_SAMPLES_FOR_P99}",
        samples.len()
    );
    samples.sort();
    let idx = ((samples.len() as f64) * 0.99).ceil() as usize;
    samples[idx.saturating_sub(1).min(samples.len() - 1)]
}

async fn upload_and_wait_ready(
    router: axum::Router,
    domain: String,
    name: String,
    rooms: usize,
) -> (Duration, Duration) {
    let payload = building_with_n_rooms(&name, &domain, rooms);
    let start = Instant::now();
    let response = send(
        router.clone(),
        "POST",
        "/register",
        Some(&admin_of(&domain)),
        Some(payload),
    )
    .await;
    let accept_latency = start.elapsed();
    assert_eq!(response.status, StatusCode::ACCEPTED);
    let id = response.body["buildingId"].as_str().unwrap().to_owned();

    let acknowledged_at = Instant::now();
    wait_for_status(&router, &id, "ready").await;
    (accept_latency, acknowledged_at.elapsed())
}

#[when(
    expr = "{int} Domain Administrators each upload a valid description of a {int}-room building to organization {string}"
)]
async fn concurrent_uploads(world: &mut TwinWorld, admins: usize, rooms: usize, domain: String) {
    let tasks = (0..admins).map(|i| {
        tokio::spawn(upload_and_wait_ready(
            world.router.clone(),
            domain.clone(),
            format!("Load Building {i}"),
            rooms,
        ))
    });

    let results: Vec<(Duration, Duration)> = futures::future::join_all(tasks)
        .await
        .into_iter()
        .map(|r| r.expect("upload task panicked"))
        .collect();

    world.load_run = Some(LoadRun {
        accept_latencies: results.iter().map(|(a, _)| *a).collect(),
        available_latencies: results.iter().map(|(_, b)| *b).collect(),
    });
}

#[then(expr = "the 99th percentile of the request-to-acknowledgement time is at most {int} second")]
async fn p99_accept_at_most(world: &mut TwinWorld, secs: u64) {
    let run = world.load_run.as_ref().expect("no load run recorded");
    let observed = p99(run.accept_latencies.clone());
    assert!(
        observed <= Duration::from_secs(secs),
        "p99 accept latency {observed:?} exceeded {secs}s"
    );
}

#[then("every twin eventually becomes available for viewing and editing")]
async fn every_twin_available(world: &mut TwinWorld) {
    let run = world.load_run.as_ref().expect("no load run recorded");
    assert!(
        !run.available_latencies.is_empty(),
        "no uploads were tracked for availability"
    );
}

#[then(
    expr = "the 99th percentile of the acknowledgement-to-available time is at most {int} seconds"
)]
async fn p99_available_at_most(world: &mut TwinWorld, secs: u64) {
    let run = world.load_run.as_ref().expect("no load run recorded");
    let observed = p99(run.available_latencies.clone());
    assert!(
        observed <= Duration::from_secs(secs),
        "p99 available latency {observed:?} exceeded {secs}s"
    );
}
