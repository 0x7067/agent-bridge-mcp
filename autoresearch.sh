#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
import re
from pathlib import Path

root = Path.cwd()
source = (root / "crates/agent-bridge-mcp/src/task/review.rs").read_text()
body = source.split("pub(super) fn next_actions", 1)[1]
body = body.split("} else {", 1)[0]
ids = re.findall(r'next_action\(\s*"([^"]+)"', body)
first = ids[0] if ids else ""
running_first_wait_final = int(first == "wait_final")
running_first_observe = int(first == "observe")
polling_friction = 0 if running_first_wait_final else 1
print(f"METRIC polling_friction={polling_friction}")
print(f"METRIC running_first_wait_final={running_first_wait_final}")
print(f"METRIC running_first_observe={running_first_observe}")
PY

cargo test -q -p agent-bridge-mcp next_actions_reflect_running_final_and_worktree_states -- --test-threads=1
