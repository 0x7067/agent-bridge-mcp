#!/usr/bin/env bash
set -euo pipefail

cargo test -q -p agent-bridge-mcp --test server_protocol
cargo build -q -p agent-bridge-mcp --bin agent-bridge-mcp

python3 - <<'PY'
import json
import os
import subprocess
import tempfile
from pathlib import Path

root = Path.cwd()
binary = root / "target" / "debug" / "agent-bridge-mcp"

def compact_size(value):
    return len(json.dumps(value, separators=(",", ":"), sort_keys=True).encode())

def request(proc, method, params, id_):
    payload = {"jsonrpc": "2.0", "id": id_, "method": method, "params": params}
    proc.stdin.write(json.dumps(payload) + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        raise RuntimeError("agent-bridge-mcp exited before responding")
    response = json.loads(line)
    if "error" in response:
        raise RuntimeError(f"{method} failed: {response['error']}")
    return response

with tempfile.TemporaryDirectory(prefix="agent-bridge-autoresearch-") as temp:
    env = os.environ.copy()
    env["AGENT_BRIDGE_WORKSPACES"] = str(root)
    env["AGENT_BRIDGE_STATE_DIR"] = str(Path(temp) / "state")
    env.setdefault("CODEX_ACP_BIN", "codex")
    proc = subprocess.Popen(
        [str(binary)],
        cwd=root,
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    try:
        sizes = {}
        next_id = 1

        initialize = request(proc, "initialize", {}, next_id)
        next_id += 1
        sizes["initialize_bytes"] = compact_size(initialize)

        tools = request(proc, "tools/list", {}, next_id)
        next_id += 1
        sizes["tools_list_bytes"] = compact_size(tools)

        prompts = request(proc, "prompts/list", {}, next_id)
        next_id += 1
        prompt_total = compact_size(prompts)
        for prompt in prompts["result"]["prompts"]:
            body = request(proc, "prompts/get", {"name": prompt["name"]}, next_id)
            next_id += 1
            prompt_total += compact_size(body)
        sizes["prompts_bytes"] = prompt_total

        resources = request(proc, "resources/list", {}, next_id)
        next_id += 1
        resource_total = compact_size(resources)
        for resource in resources["result"]["resources"]:
            body = request(proc, "resources/read", {"uri": resource["uri"]}, next_id)
            next_id += 1
            resource_total += compact_size(body)
        sizes["resources_bytes"] = resource_total

        providers = request(
            proc,
            "tools/call",
            {"name": "providers_list", "arguments": {}},
            next_id,
        )
        next_id += 1
        sizes["providers_list_bytes"] = compact_size(providers)

        dryrun = request(
            proc,
            "tools/call",
            {
                "name": "agent_spawn",
                "arguments": {
                    "provider": "codex",
                    "mode": "research",
                    "prompt": "Find one cost-efficiency issue.",
                    "cwd": str(root),
                    "dryRun": True,
                },
            },
            next_id,
        )
        sizes["dryrun_bytes"] = compact_size(dryrun)
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=2)

total = sum(sizes.values())
print(f"METRIC total_bytes={total}")
for name in sorted(sizes):
    print(f"METRIC {name}={sizes[name]}")
PY
