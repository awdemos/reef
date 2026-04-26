pub mod error;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod state;

pub use error::{ApiError, ApiResult};
pub use middleware::{auth_middleware, AuthPrincipal};
pub use models::*;
pub use routes::build_router;
pub use state::{AppState, ServerConfig};
