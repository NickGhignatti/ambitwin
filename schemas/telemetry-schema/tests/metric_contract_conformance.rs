use telemetry_schema::{Aggregation, MetricsDiscoveryResponse, ServiceMetricsContract};

const FIXTURE: &str = include_str!("../../fixtures/metric-contract.json");

// The fixture is what the frontend and the acceptance suite read; they cannot share this
// crate's types, so these bytes are the only thing holding all three languages together.
#[test]
fn the_fixture_parses_into_the_catalog_every_service_serves() {
    let catalog: ServiceMetricsContract = serde_json::from_str(FIXTURE).expect("fixture parses");

    assert_eq!(catalog.service, "telemetry");
    let kinds: Vec<&str> = catalog.metrics.iter().map(|m| m.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "temperature",
            "people_count",
            "co2",
            "indoor_aqi",
            "humidity",
            "total_device_count",
            "ratio_device_count",
            "estimated_occupancy"
        ]
    );
}

// A rename on either side changes these bytes; nothing else in Rust would notice, because
// both producers and the consumer build from the same struct and would still agree.
#[test]
fn re_serialising_the_fixture_reproduces_it_byte_for_byte() {
    let parsed: ServiceMetricsContract = serde_json::from_str(FIXTURE).expect("fixture parses");
    let original: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");

    assert_eq!(serde_json::to_value(parsed).unwrap(), original);
}

#[test]
fn the_wire_names_are_the_single_words_the_frontend_reads() {
    let raw: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
    let metric = &raw["metrics"][0];

    assert_eq!(metric["kind"], "temperature");
    assert_eq!(metric["interface"], "ITemperature");
    assert_eq!(metric["source"], "telemetry");
    assert_eq!(metric["actions"][0]["parameters"][0]["type"], "Finite");
}

// A unitless metric sends null rather than omitting the key, so a client can tell
// "no unit" from "this producer is too old to send one".
#[test]
fn a_unitless_metric_carries_an_explicit_null() {
    let raw: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
    let indoor_aqi = raw["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|metric| metric["kind"] == "indoor_aqi")
        .expect("the fixture covers a unitless metric");

    assert_eq!(indoor_aqi["unit"], serde_json::Value::Null);
}

// A client offers only the ends a metric lists: none for a device count, a max with no start
// for people (it comes from each room's capacity), both with a start for temperature.
#[test]
fn a_limit_says_which_ends_a_metric_takes_and_where_they_start() {
    let catalog: ServiceMetricsContract = serde_json::from_str(FIXTURE).expect("fixture parses");
    let limit_of = |kind: &str| {
        &catalog
            .metrics
            .iter()
            .find(|metric| metric.kind == kind)
            .unwrap()
            .limit
    };

    assert_eq!(limit_of("total_device_count").min, None);
    assert_eq!(limit_of("total_device_count").max, None);
    assert_eq!(limit_of("people_count").max, Some(None));
    assert_eq!(limit_of("people_count").min, None);
    assert_eq!(limit_of("temperature").min, Some(Some(18.0)));
    assert_eq!(limit_of("temperature").max, Some(Some(27.0)));
}

// a building card adds counts and averages measures; guessing it from the name is how a sum gets averaged
#[test]
fn every_telemetry_metric_declares_how_its_rooms_add_up() {
    let catalog: ServiceMetricsContract = serde_json::from_str(FIXTURE).expect("fixture parses");
    let aggregation_of = |kind: &str| {
        catalog
            .metrics
            .iter()
            .find(|metric| metric.kind == kind)
            .unwrap()
            .aggregation
    };

    assert_eq!(aggregation_of("temperature"), Some(Aggregation::Average));
    assert_eq!(aggregation_of("people_count"), Some(Aggregation::Sum));
    assert!(
        catalog
            .metrics
            .iter()
            .all(|metric| metric.aggregation.is_some())
    );
}

#[test]
fn the_whole_fixture_decodes_as_the_service_variant() {
    let decoded: MetricsDiscoveryResponse =
        serde_json::from_str(FIXTURE).expect("fixture parses as a discovery response");

    assert!(matches!(
        decoded,
        MetricsDiscoveryResponse::ServiceContract(_)
    ));
}

// The same bytes minus the wrapper are the bare-array answer a source may send instead, and
// the array dashboard hands the frontend. One fixture therefore covers both hops.
#[test]
fn the_fixtures_metrics_alone_decode_as_the_bare_array_variant() {
    let raw: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
    let decoded: MetricsDiscoveryResponse =
        serde_json::from_value(raw["metrics"].clone()).expect("a bare array is a valid answer");

    match decoded {
        MetricsDiscoveryResponse::Metrics(metrics) => assert_eq!(metrics.len(), 8),
        MetricsDiscoveryResponse::ServiceContract(_) => {
            panic!("a bare array must not decode as the service variant")
        }
    }
}
