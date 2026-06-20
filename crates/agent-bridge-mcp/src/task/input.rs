use crate::domain::{Isolation, LaunchProfile, ProviderKind, RetryPolicy, TaskMode};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPreviewInput {
    pub provider: ProviderKind,
    pub mode: TaskMode,
    pub prompt: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub thinking: Option<String>,
    pub isolation: Option<Isolation>,
    pub worktree_name: Option<String>,
    pub profile: Option<LaunchProfile>,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub retry_policy: Option<RetryPolicy>,
}
