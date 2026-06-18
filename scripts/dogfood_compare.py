#!/usr/bin/env python3
"""Run the same read-only prompt through bridge and bare launch profiles.

The harness talks to the local Agent Bridge MCP server over stdio and writes
one evidence directory per provider/profile pair.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

PROFILES = ("bridge", "bare")
RESULT_SECTIONS = ["summary", "stdout", "stderr", "transcript"]
REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PROMPT_FILE = REPO_ROOT / "examples" / "dogfood" / "read-only-prompt.md"


@dataclass(frozen=True)
class RunSpec:
    provider: str
    profile: str


@dataclass(frozen=True)
class RunConfig:
    cwd: str
    prompt: str
    mode: str
    timeout_seconds: int
    observe_timeout_ms: int
    transcript_limit: int
    result_max_bytes: int
    dry_run: bool = False


class McpError(RuntimeError):
    pass


class StdioMcpClient:
    def __init__(self, command: list[str], env: dict[str, str], stderr_path: Path):
        self.command = command
        self.env = env
        self.stderr_path = stderr_path
        self.process: subprocess.Popen[str] | None = None
        self.next_id = 1
        self.stderr_file = None

    def __enter__(self) -> "StdioMcpClient":
        self.stderr_path.parent.mkdir(parents=True, exist_ok=True)
        self.stderr_file = self.stderr_path.open("w", encoding="utf-8")
        self.process = subprocess.Popen(
            self.command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self.stderr_file,
            text=True,
            encoding="utf-8",
            env=self.env,
        )
        self.request("initialize", {})
        self.notify("notifications/initialized", {})
        return self

    def __exit__(self, _exc_type, _exc, _tb) -> None:
        if self.process is not None:
            if self.process.stdin is not None:
                self.process.stdin.close()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.terminate()
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=5)
        if self.stderr_file is not None:
            self.stderr_file.close()

    def request(self, method: str, params: dict[str, Any]) -> dict[str, Any]:
        if self.process is None or self.process.stdin is None or self.process.stdout is None:
            raise McpError("MCP server is not running")
        request_id = self.next_id
        self.next_id += 1
        payload = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        }
        self.process.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        if not line:
            raise McpError(f"MCP server exited before responding to {method}")
        response = json.loads(line)
        if response.get("id") != request_id:
            raise McpError(f"unexpected MCP response id for {method}: {response}")
        if "error" in response:
            raise McpError(f"{method} failed: {response['error']}")
        return response

    def notify(self, method: str, params: dict[str, Any]) -> None:
        if self.process is None or self.process.stdin is None:
            raise McpError("MCP server is not running")
        payload = {"jsonrpc": "2.0", "method": method, "params": params}
        self.process.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
        self.process.stdin.flush()

    def tool(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        response = self.request(
            "tools/call",
            {
                "name": name,
                "arguments": arguments,
            },
        )
        result = response["result"]
        if result.get("isError") is True:
            text = result.get("content", [{}])[0].get("text", "")
            raise McpError(f"{name} returned tool error: {text}")
        text = result["content"][0]["text"]
        return json.loads(text)


def build_run_matrix(providers: list[str]) -> list[RunSpec]:
    return [RunSpec(provider=provider, profile=profile) for provider in providers for profile in PROFILES]


def failed_runs(runs: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [run for run in runs if run.get("status") != "succeeded"]


def run_one(client: Any, run: RunSpec, config: RunConfig, output_dir: Path) -> dict[str, Any]:
    run_dir = output_dir / "runs" / run.provider / run.profile
    run_dir.mkdir(parents=True, exist_ok=True)

    spawn_args = {
        "provider": run.provider,
        "profile": run.profile,
        "mode": config.mode,
        "prompt": config.prompt,
        "cwd": config.cwd,
        "isolation": "none",
        "timeoutSeconds": config.timeout_seconds,
        "title": f"dogfood {run.provider} {run.profile}",
    }
    if config.dry_run:
        spawn_args["dryRun"] = True
    spawn = client.tool("agent_spawn", spawn_args)
    write_json(run_dir / "agent_spawn.json", spawn)
    if config.dry_run:
        return {
            "provider": run.provider,
            "profile": run.profile,
            "status": spawn.get("status", "preview"),
            "dryRun": True,
            "spawnPath": str(run_dir / "agent_spawn.json"),
        }
    agent_id = spawn["agentId"]

    observe = client.tool(
        "agent_observe",
        {
            "agentId": agent_id,
            "until": "final",
            "timeoutMs": config.observe_timeout_ms,
            "limit": config.transcript_limit,
            "verbosity": "detailed",
        },
    )
    write_json(run_dir / "agent_observe.json", observe)

    result = client.tool(
        "agent_result",
        {
            "agentId": agent_id,
            "sections": RESULT_SECTIONS,
            "cursor": 0,
            "limit": config.transcript_limit,
            "maxBytes": config.result_max_bytes,
            "verbosity": "detailed",
        },
    )
    write_json(run_dir / "task_result.json", result)
    write_json(run_dir / "task_transcript.json", result.get("transcript", {}))

    return {
        "provider": run.provider,
        "profile": run.profile,
        "agentId": agent_id,
        "status": result.get("status", observe.get("status")),
        "transcriptAvailable": result.get("transcript", {}).get("available"),
        "resultPath": str(run_dir / "task_result.json"),
        "transcriptPath": str(run_dir / "task_transcript.json"),
    }


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def read_prompt(args: argparse.Namespace) -> str:
    if args.prompt:
        return args.prompt
    return Path(args.prompt_file).read_text(encoding="utf-8")


def parse_providers(raw: str) -> list[str]:
    providers = [provider.strip() for provider in raw.split(",") if provider.strip()]
    if not providers:
        raise argparse.ArgumentTypeError("at least one provider is required")
    return providers


def default_server_command() -> list[str]:
    env_bin = os.environ.get("AGENT_BRIDGE_MCP_BIN")
    if env_bin:
        return [env_bin]
    debug_bin = REPO_ROOT / "target" / "debug" / "agent-bridge-mcp"
    if debug_bin.exists():
        return [str(debug_bin)]
    path_bin = shutil.which("agent-bridge-mcp")
    if path_bin:
        return [path_bin]
    return [str(debug_bin)]


def build_env(cwd: str, strict_validation: bool = False) -> dict[str, str]:
    env = os.environ.copy()
    env.setdefault("AGENT_BRIDGE_WORKSPACES", cwd)
    if strict_validation:
        env["AGENT_BRIDGE_STRICT_VALIDATION"] = "true"
    return env


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--providers",
        type=parse_providers,
        default=["codex"],
        help="Comma-separated providers to compare. Example: codex,cursor,kimi",
    )
    parser.add_argument(
        "--server",
        nargs="+",
        default=default_server_command(),
        help="MCP server command. Default: AGENT_BRIDGE_MCP_BIN, target/debug/agent-bridge-mcp, or PATH.",
    )
    parser.add_argument("--cwd", default=str(REPO_ROOT), help="Workspace cwd passed to agent_spawn.")
    parser.add_argument("--prompt-file", default=str(DEFAULT_PROMPT_FILE), help="Read-only prompt file.")
    parser.add_argument("--prompt", help="Inline prompt. Overrides --prompt-file.")
    parser.add_argument("--mode", default="research", choices=["research", "review", "command"])
    parser.add_argument("--timeout-seconds", type=int, default=120)
    parser.add_argument("--observe-timeout-ms", type=int, default=180_000)
    parser.add_argument("--transcript-limit", type=int, default=400)
    parser.add_argument("--result-max-bytes", type=int, default=200_000)
    parser.add_argument(
        "--strict-validation",
        action="store_true",
        help="Run the server with AGENT_BRIDGE_STRICT_VALIDATION=true.",
    )
    parser.add_argument(
        "--require-success",
        action="store_true",
        help="Exit 1 if any provider/profile run does not finish with status=succeeded.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Preview agent_spawn requests without launching providers.",
    )
    parser.add_argument(
        "--output-dir",
        default=None,
        help="Evidence directory. Default: artifacts/dogfood/<UTC timestamp>",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    output_dir = Path(args.output_dir or REPO_ROOT / "artifacts" / "dogfood" / timestamp)
    output_dir.mkdir(parents=True, exist_ok=True)

    config = RunConfig(
        cwd=str(Path(args.cwd).resolve()),
        prompt=read_prompt(args),
        mode=args.mode,
        timeout_seconds=args.timeout_seconds,
        observe_timeout_ms=args.observe_timeout_ms,
        transcript_limit=args.transcript_limit,
        result_max_bytes=args.result_max_bytes,
        dry_run=args.dry_run,
    )
    matrix = build_run_matrix(args.providers)
    manifest = {
        "generatedAt": timestamp,
        "server": args.server,
        "cwd": config.cwd,
        "mode": config.mode,
        "providers": args.providers,
        "profiles": list(PROFILES),
        "promptFile": None if args.prompt else str(Path(args.prompt_file).resolve()),
        "dryRun": args.dry_run,
        "requireSuccess": args.require_success,
        "strictValidation": args.strict_validation,
        "runs": [],
    }

    with StdioMcpClient(
        args.server,
        build_env(config.cwd, args.strict_validation),
        output_dir / "server_stderr.log",
    ) as client:
        for run in matrix:
            summary = run_one(client, run, config, output_dir)
            manifest["runs"].append(summary)
            evidence_path = summary.get("resultPath", summary.get("spawnPath"))
            print(f"{run.provider}/{run.profile}: {summary['status']} -> {evidence_path}")

    write_json(output_dir / "manifest.json", manifest)
    print(f"manifest: {output_dir / 'manifest.json'}")
    failures = failed_runs(manifest["runs"])
    if args.require_success and failures:
        for failure in failures:
            evidence_path = failure.get("resultPath", failure.get("spawnPath"))
            print(
                f"failed: {failure['provider']}/{failure['profile']}: {failure['status']} -> {evidence_path}",
                file=sys.stderr,
            )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
