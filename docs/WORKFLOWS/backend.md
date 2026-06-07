# Backend Workflows

**Last Updated:** 2026-06-07
**Based on patterns from:** `src/tools.rs`, `src/server.rs`, `src/provider.rs`, `tests/server_protocol.rs`

## How to Add a New Tool or Extend an Existing One

### Step 1: Register the Tool Name

Add a discriminant to `ToolName` in `src/tools.rs`:

```rust
// src/tools.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolName {
    // ...existing...
    #[serde(rename = "my_new_tool")]
    MyNewTool,
}
```

### Step 2: Define the Input Schema

Define a strongly-typed input struct with `deny_unknown_fields`:

```rust
// src/tools.rs
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MyNewToolInput {
    pub required_field: String,
    pub optional_flag: Option<bool>,
}
```

Wire the schema into `tool_definitions()`:

```rust
pub fn tool_definitions() -> Vec<Value> {
    let mut defs = vec![ /* existing */ ];
    defs.push(json!({
        "name": "my_new_tool",
        "description": "What it does.",
        "inputSchema": object_schema(
            json!({
                "requiredField": {"type": "string"},
                "optionalFlag": {"type": "boolean"}
            }),
            vec!["requiredField"]
        ),
        "annotations": read_only_annotations("Description")
    }));
    defs
}
```

### Step 3: Dispatch in the Server Router

Add a match arm in `src/server.rs` `call_tool()`:

```rust
ToolName::MyNewTool => {
    match parse_my_new_tool_params(&params.arguments) {
        Ok(input) => tool_json(my_new_tool_logic(input).await),
        Err(error) => tool_error(error),
    }
}
```

### Step 4: Validate and Reject Unknown Fields

Ensure `reject_unknown_arguments(params.name, &params.arguments)` is called before your logic. It already runs centrally in `call_tool()`.

### Step 5: Test End-to-End

Add a test in `tests/server_protocol.rs` mirroring the existing `tools_call` tests:

```rust
#[test]
fn my_new_tool_happy_path() {
    let response = call_tool_sync(
        "my_new_tool",
        json!({"requiredField": "hello"})
    );
    assert_eq!(response["status"], "success");
}
```

Run with:

```bash
cargo test --test server_protocol -- my_new_tool --nocapture
```

## How to Add a New Provider Adapter

### Step 1: Declare Support in Capabilities

Edit `provider.rs` `capabilities()` and add a new top-level key:

```rust
"new_provider": {
    "modes": ["research", "review"],
    "supportsReply": false,
    "supportsResume": false,
    "supportsWorktreeIsolation": true,
    "launchProfiles": ["bridge", "bare"],
    "presentationActions": presentation_actions(),
    "outputCadence": output_cadence(ProviderKind::NewProvider),
    "readiness": default_readiness()
}
```

### Step 2: Wire the Provider Enum

Add `NewProvider` to `ProviderKind` in `domain.rs` and derive the `as_str()` mapping. Also update `provider_names()`.

### Step 3: Implement Command Construction

Extend `provider.rs` `prepare_command()` (or equivalent) to recognize the new provider and build its `ProviderCommand`:

```rust
ProviderKind::NewProvider => {
    ProviderCommand {
        provider: ProviderKind::NewProvider,
        command: "new-provider-cli".to_string(),
        args: vec!["exec".to_string(), "--prompt".to_string(), prompt.to_string()],
        stdin: None,
        redactions: vec![],
        cwd: cwd.to_string(),
        timeout_seconds,
        env: provider_env(ProviderKind::NewProvider),
        profile,
        prompt_strategy: "inline_arg".to_string(),
        profile_diagnostics: json!({}),
        ..Default::default()
    }
}
```

### Step 4: Add Denial Detection (Optional)

If the provider emits recognizable stderr on failure, add an adapter method:

```rust
fn detects_fatal_denial(&self, stderr: &[u8]) -> bool {
    String::from_utf8_lossy(stderr).contains("SANDBOX_DENIED")
}
```

Register the adapter in `adapter_for()`.

### Step 5: Add Fixtures and Smoke Tests

Provide a fake script in `tests/fixtures/new_provider_fake.sh` and wire it into `doctor` smoke tests and protocol tests.

## Running Quality Gates Locally

```bash
./scripts/quality.sh
```

Hard gates (script exits non-zero on failure):
- `cargo fmt --all --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo machete`
- `npx jscpd`

Informational only:
- Complexity hotspot warnings
- Module dependency graph (acyclic + boundary review)

Fix any hard gate violation before opening a PR.
