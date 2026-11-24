use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::infra::authz::{require_edit, require_read};
use crate::infra::claims::GatewayClaims;
use crate::infra::db;
use crate::state::AppState;

const MAX_CARDS: usize = 32;
const MAX_CARD_ID_LENGTH: usize = 64;

/// A building's dashboard cards, in display order: metric kinds or `rooms` / `alerts`.
#[derive(Debug, Serialize, Deserialize)]
pub struct CardsBody {
    pub cards: Vec<String>,
}

/// `GET /preferences/{building_id}/cards`: the saved cards, or `404` when nobody chose any,
/// so the client shows its default. Needs `Read` on the building.
pub async fn get_cards(
    claims: GatewayClaims,
    Path(building_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    if let Err(status) = require_read(&state.twin, &claims, &building_id).await {
        return status.into_response();
    }
    match state.building_cards.get(&building_id) {
        Some(cards) => Json(CardsBody {
            cards: cards.clone(),
        })
        .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `PUT /preferences/{building_id}/cards`: replaces the building's cards for every viewer.
/// Needs `Edit` on the building. `400` on a list that is too long, has an empty or overlong id,
/// or names a card twice.
pub async fn put_cards(
    claims: GatewayClaims,
    State(state): State<AppState>,
    Path(building_id): Path<String>,
    Json(body): Json<CardsBody>,
) -> Response {
    if let Err(status) = require_edit(&state.twin, &claims, &building_id).await {
        return status.into_response();
    }
    if let Some(problem) = find_problem(&body.cards) {
        return (StatusCode::BAD_REQUEST, problem).into_response();
    }
    state
        .building_cards
        .insert(building_id.clone(), body.cards.clone());

    let collection = state.mongo_col.clone();
    tokio::spawn(async move {
        if let Err(error) = db::upsert_cards(&collection, &building_id, &body.cards).await {
            log::error!("Failed to persist cards for {building_id}: {error}");
        }
    });
    StatusCode::NO_CONTENT.into_response()
}

fn find_problem(cards: &[String]) -> Option<String> {
    if cards.len() > MAX_CARDS {
        return Some(format!("at most {MAX_CARDS} cards"));
    }
    if cards
        .iter()
        .any(|card| card.is_empty() || card.len() > MAX_CARD_ID_LENGTH)
    {
        return Some(format!("a card id is 1 to {MAX_CARD_ID_LENGTH} characters"));
    }
    let mut seen = std::collections::HashSet::new();
    cards
        .iter()
        .find(|card| !seen.insert(card.as_str()))
        .map(|card| format!("card {card} appears twice"))
}

#[cfg(test)]
mod tests {
    use super::{CardsBody, MAX_CARDS, get_cards, put_cards};
    use crate::api::testing::{claims, claims_in, make_state};
    use crate::infra::claims::GatewayClaims;
    use crate::state::AppState;
    use axum::Json;
    use axum::extract::{Path, State};
    use axum::http::StatusCode;

    async fn put_as(caller: GatewayClaims, state: &AppState, cards: &[&str]) -> StatusCode {
        let body = CardsBody {
            cards: cards.iter().map(|card| card.to_string()).collect(),
        };
        put_cards(
            caller,
            State(state.clone()),
            Path("b1".to_string()),
            Json(body),
        )
        .await
        .status()
    }

    async fn put(state: &AppState, cards: &[&str]) -> StatusCode {
        put_as(claims(), state, cards).await
    }

    #[tokio::test]
    async fn a_building_nobody_customised_has_no_cards_so_the_client_shows_its_default() {
        let (state, _twin) = make_state().await;
        let response = get_cards(claims(), Path("b1".to_string()), State(state)).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn saved_cards_keep_their_order() {
        let (state, _twin) = make_state().await;
        assert_eq!(
            put(&state, &["alerts", "temperature", "rooms"]).await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            *state.building_cards.get("b1").unwrap(),
            vec!["alerts", "temperature", "rooms"]
        );
    }

    #[tokio::test]
    async fn a_card_named_twice_or_too_many_cards_are_refused_and_nothing_is_stored() {
        let (state, _twin) = make_state().await;
        let too_many: Vec<String> = (0..=MAX_CARDS).map(|i| format!("m{i}")).collect();
        let too_many: Vec<&str> = too_many.iter().map(String::as_str).collect();

        assert_eq!(put(&state, &["co2", "co2"]).await, StatusCode::BAD_REQUEST);
        assert_eq!(put(&state, &[""]).await, StatusCode::BAD_REQUEST);
        assert_eq!(put(&state, &too_many).await, StatusCode::BAD_REQUEST);
        assert!(state.building_cards.get("b1").is_none());
    }

    #[tokio::test]
    async fn an_outsider_cannot_read_the_cards_and_a_customer_cannot_change_them() {
        let (state, _twin) = make_state().await;
        state
            .building_cards
            .insert("b1".to_string(), vec!["rooms".to_string()]);

        let outsider_read = get_cards(
            claims_in("sales", "business_admin"),
            Path("b1".to_string()),
            State(state.clone()),
        )
        .await;
        let customer_save = put_as(claims_in("eng", "standard_customer"), &state, &["co2"]).await;

        assert_eq!(outsider_read.status(), StatusCode::FORBIDDEN);
        assert_eq!(customer_save, StatusCode::FORBIDDEN);
        assert_eq!(*state.building_cards.get("b1").unwrap(), vec!["rooms"]);
    }
}
