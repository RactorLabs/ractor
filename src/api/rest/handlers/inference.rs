use std::sync::Arc;

use axum::{
    extract::{Extension, State},
    Json,
};

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::api::rest::rbac_enforcement::{check_api_permission, permissions};
use crate::shared::inference::InferenceProviderInfo;
use crate::shared::models::AppState;

pub async fn list_providers(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<Vec<InferenceProviderInfo>>> {
    check_api_permission(&auth, &state, &permissions::SANDBOX_LIST)
        .await
        .map_err(|_| ApiError::Forbidden("Insufficient permissions".into()))?;
    let providers = state.inference_registry.providers().to_vec();
    Ok(Json(providers))
}
