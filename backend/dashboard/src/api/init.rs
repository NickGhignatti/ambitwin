use crate::infra::authz::require_edit;
use crate::infra::claims::GatewayClaims;
use crate::infra::db;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
};

const DEFAULT_COLUMNS: &[&str] = &["room_name", "room_max_occupancy"];

/// `POST /preferences/init/{building_id}`: gives a new building its starting columns, called by
/// digital-twin with the creator's claims. Needs `Edit`: an entry makes the tunnel forward the
/// building's ticks. A building that already has columns keeps them.
pub async fn init_building_preferences(
    claims: GatewayClaims,
    Path(building_id): Path<String>,
    State(state): State<AppState>,
) -> StatusCode {
    if let Err(status) = require_edit(&state.twin, &claims, &building_id).await {
        return status;
    }
    let mut is_new = false;
    state
        .building_preferences
        .entry(building_id.clone())
        .or_insert_with(|| {
            is_new = true;
            DEFAULT_COLUMNS.iter().map(|s| s.to_string()).collect()
        });

    if is_new {
        let defaults: Vec<String> = DEFAULT_COLUMNS.iter().map(|s| s.to_string()).collect();
        let col = state.mongo_col.clone();
        tokio::spawn(async move {
            if let Err(e) = db::upsert_preference(&col, &building_id, &defaults).await {
                log::error!("Failed to persist init preference for {building_id}: {e}");
            }
        });
    }

    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_COLUMNS, init_building_preferences};
    use crate::api::testing::{claims, claims_in, make_state, state_with};
    use crate::infra::claims::GatewayClaims;
    use crate::state::AppState;
    use axum::extract::{Path, State};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    async fn init(claims: GatewayClaims, building_id: &str, state: &AppState) -> StatusCode {
        init_building_preferences(claims, Path(building_id.to_string()), State(state.clone()))
            .await
            .into_response()
            .status()
    }

    #[tokio::test]
    async fn new_building_receives_default_columns() {
        let (state, _twin) = make_state().await;
        assert_eq!(init(claims(), "bldg-1", &state).await, StatusCode::OK);

        let cols = state.building_preferences.get("bldg-1").unwrap();
        assert!(cols.contains(&"room_name".to_string()));
        assert!(cols.contains(&"room_max_occupancy".to_string()));
        assert_eq!(cols.len(), DEFAULT_COLUMNS.len());
    }

    #[tokio::test]
    async fn second_init_for_same_building_is_idempotent() {
        let (state, _twin) = make_state().await;
        state
            .building_preferences
            .insert("bldg-1".to_string(), vec!["custom-col".to_string()]);

        init(claims(), "bldg-1", &state).await;

        let cols = state.building_preferences.get("bldg-1").unwrap();
        assert_eq!(*cols, vec!["custom-col"]);
    }

    #[tokio::test]
    async fn two_buildings_are_initialised_independently() {
        let (state, _twin) = make_state().await;
        init(claims(), "bldg-a", &state).await;
        init(claims(), "bldg-b", &state).await;

        assert!(state.building_preferences.contains_key("bldg-a"));
        assert!(state.building_preferences.contains_key("bldg-b"));
        assert_eq!(state.building_preferences.len(), 2);
    }

    // an entry makes the tunnel forward the building's ticks, so only its editors may create one
    #[tokio::test]
    async fn someone_who_may_not_edit_the_building_cannot_start_its_columns() {
        let (state, _twin) = make_state().await;
        for caller in [
            claims_in("sales", "business_admin"),
            claims_in("eng", "standard_customer"),
        ] {
            assert_eq!(init(caller, "bldg-1", &state).await, StatusCode::FORBIDDEN);
        }
        assert!(state.building_preferences.is_empty());
    }

    #[tokio::test]
    async fn no_columns_are_started_when_digital_twin_cannot_say_who_may_edit() {
        let (state, _twin) = state_with(true).await;
        assert_eq!(
            init(claims(), "bldg-1", &state).await,
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert!(state.building_preferences.is_empty());
    }

    #[tokio::test]
    async fn default_columns_contain_exactly_room_name_and_max_occupancy() {
        assert!(DEFAULT_COLUMNS.contains(&"room_name"));
        assert!(DEFAULT_COLUMNS.contains(&"room_max_occupancy"));
        assert_eq!(DEFAULT_COLUMNS.len(), 2);
    }
}
