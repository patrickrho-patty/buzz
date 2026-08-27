use serde::Serialize;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyWorkspaceStorage {
    workspaces: Option<String>,
    active_workspace_id: Option<String>,
    onboarding_completions: Vec<LegacyOnboardingCompletion>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyOnboardingCompletion {
    pubkey: String,
    value: String,
}

/// Return workspace-scoped localStorage values from pre-fork installs.
///
/// Fresh-fork posture: no pre-fork installs exist, so this always
/// returns empty defaults. Kept as a command so the frontend seeding path
/// stays unchanged and can be revived if pre-fork data ever needs import.
#[tauri::command]
pub async fn get_legacy_workspace_storage(
    _app: tauri::AppHandle,
) -> Result<LegacyWorkspaceStorage, String> {
    Ok(LegacyWorkspaceStorage::default())
}
