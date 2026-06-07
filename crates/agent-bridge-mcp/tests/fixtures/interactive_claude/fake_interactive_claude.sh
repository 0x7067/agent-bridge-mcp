#!/bin/sh
set -eu

scenario="${FAKE_CLAUDE_SCENARIO:-success}"
fixture_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
tmp_root="${TMPDIR:-/tmp}/agent-bridge-fake-claude-$$"
mkdir -p "$tmp_root"

hook_sink="${AGENT_BRIDGE_FAKE_CLAUDE_HOOK_SINK:-}"
prompt_log="${AGENT_BRIDGE_FAKE_CLAUDE_PROMPT_LOG:-$tmp_root/prompt.txt}"
cleanup_marker="${AGENT_BRIDGE_FAKE_CLAUDE_CLEANUP_MARKER:-$tmp_root/cleanup.txt}"

emit_hook() {
  event="$1"
  payload="$2"
  if [ -n "$hook_sink" ]; then
    printf '%s\t%s\n' "$event" "$payload" >> "$hook_sink"
  else
    printf '%s\t%s\n' "$event" "$payload"
  fi
}

emit_session_start() {
  transcript_path="$1"
  emit_hook "SessionStart" "{\"session_id\":\"fake-session\",\"transcript_path\":\"$transcript_path\",\"cwd\":\"$PWD\",\"hook_event_name\":\"SessionStart\"}"
}

emit_stop() {
  transcript_path="$1"
  emit_hook "Stop" "{\"session_id\":\"fake-session\",\"transcript_path\":\"$transcript_path\",\"cwd\":\"$PWD\",\"hook_event_name\":\"Stop\",\"stop_hook_active\":false,\"last_assistant_message\":\"fixture final response\",\"background_tasks\":[],\"session_crons\":[]}"
}

# Mirror the real hook relay helper, which emits a bare Stop payload (no
# transcript_path / last_assistant_message) so completion must resolve via the
# SessionStart transcript_path.
emit_bare_stop() {
  emit_hook "Stop" "{\"hook_event_name\":\"Stop\"}"
}

capture_prompt() {
  mkdir -p "$(dirname -- "$prompt_log")"
  prompt=""
  if IFS= read -r prompt; then
    printf '%s' "$prompt" > "$prompt_log"
  else
    : > "$prompt_log"
  fi
}

case "$scenario" in
  terminal-probes)
    printf '\033[c\033[>c\033[6n\033[>q\033[18t'
    ;;
  prompt-entry)
    printf 'prompt-entry-ready\n'
    capture_prompt
    printf 'prompt captured\n'
    ;;
  success)
    transcript_path="$tmp_root/success.jsonl"
    cp "$fixture_dir/transcripts/success.jsonl" "$transcript_path"
    emit_session_start "$transcript_path"
    capture_prompt
    emit_stop "$transcript_path"
    ;;
  stop-stays-open)
    transcript_path="$tmp_root/success.jsonl"
    cp "$fixture_dir/transcripts/success.jsonl" "$transcript_path"
    emit_session_start "$transcript_path"
    capture_prompt
    emit_stop "$transcript_path"
    sleep 30
    ;;
  premature-stop)
    # Claude fires a Stop hook at the end of every turn. The first Stop here
    # lands before any assistant turn exists in the transcript (premature); the
    # runner must ignore it and keep the session alive. After the assistant turn
    # is written, a second Stop must be accepted and resolved.
    transcript_path="$tmp_root/premature.jsonl"
    cp "$fixture_dir/transcripts/no_assistant.jsonl" "$transcript_path"
    emit_session_start "$transcript_path"
    capture_prompt
    emit_bare_stop
    # Outlast TRANSCRIPT_RETRY_BUDGET (2s) so the premature resolve attempt fails
    # before the real answer is written.
    sleep 3
    cp "$fixture_dir/transcripts/success.jsonl" "$transcript_path"
    emit_bare_stop
    sleep 30
    ;;
  malformed-transcript)
    transcript_path="$tmp_root/malformed.jsonl"
    cp "$fixture_dir/transcripts/malformed.jsonl" "$transcript_path"
    emit_session_start "$transcript_path"
    capture_prompt
    emit_stop "$transcript_path"
    ;;
  stop-failure-rate-limit)
    cat "$fixture_dir/hooks/stop_failure_rate_limit.json"
    printf '\n'
    ;;
  stop-failure-auth)
    cat "$fixture_dir/hooks/stop_failure_auth.json"
    printf '\n'
    ;;
  setup-login)
    cat "$fixture_dir/setup_prompts/login.txt"
    sleep 2
    ;;
  setup-trust)
    cat "$fixture_dir/setup_prompts/workspace_trust.txt"
    sleep 2
    ;;
  timeout)
    printf 'fake claude waiting for hook completion\n'
    sleep 30
    ;;
  child-cleanup)
    sh -c 'trap "printf child-terminated > \"$1\"; exit 0" TERM INT; while :; do sleep 1; done' sh "$cleanup_marker" &
    child=$!
    trap 'kill -TERM "$child" 2>/dev/null || true; wait "$child" 2>/dev/null || true; printf parent-terminated >> "$cleanup_marker"; exit 143' TERM INT
    wait "$child"
    ;;
  *)
    printf 'unknown fake Claude scenario: %s\n' "$scenario" >&2
    exit 64
    ;;
esac
