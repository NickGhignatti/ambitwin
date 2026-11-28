use std::sync::Arc;

use crate::service::conversations::Conversations;
use rate_limit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub conversations: Arc<Conversations>,
    pub rate_limiter: RateLimiter,
}
