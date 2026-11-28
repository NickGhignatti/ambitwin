use std::sync::Arc;

use crate::service::buildings::Buildings;
use crate::service::placements::Placements;
use crate::service::provisioning::Provisioning;
use rate_limit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub buildings: Arc<Buildings>,
    pub placements: Arc<Placements>,
    pub provisioning: Arc<Provisioning>,
    pub rate_limiter: RateLimiter,
}
