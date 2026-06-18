import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import dogfood_compare


class FakeMcpClient:
    def __init__(self):
        self.calls = []
        self.next_agent = 1

    def tool(self, name, arguments):
        self.calls.append((name, arguments))
        if name == "agent_spawn":
            agent_id = f"agent_{self.next_agent}"
            self.next_agent += 1
            return {
                "agentId": agent_id,
                "provider": arguments["provider"],
                "profile": arguments["profile"],
                "status": "running",
            }
        if name == "agent_observe":
            return {
                "agentId": arguments["agentId"],
                "status": "succeeded",
                "events": [{"kind": "lifecycle", "parsed": {"phase": "completed"}}],
                "nextCursor": 1,
            }
        if name == "agent_result":
            return {
                "agentId": arguments["agentId"],
                "status": "succeeded",
                "reviewPacket": {
                    "profile": "bare",
                    "transcriptAvailable": True,
                    "finalResultDetected": True,
                },
                "transcript": {
                    "available": True,
                    "events": [{"kind": "provider_result"}],
                    "nextCursor": 1,
                },
            }
        raise AssertionError(f"unexpected tool call: {name}")


class DogfoodCompareTests(unittest.TestCase):
    def test_build_run_matrix_pairs_each_provider_with_bridge_and_bare(self):
        matrix = dogfood_compare.build_run_matrix(["codex", "cursor"])

        self.assertEqual(
            [(run.provider, run.profile) for run in matrix],
            [
                ("codex", "bridge"),
                ("codex", "bare"),
                ("cursor", "bridge"),
                ("cursor", "bare"),
            ],
        )

    def test_run_one_captures_spawn_observe_transcript_and_result_artifacts(self):
        client = FakeMcpClient()
        run = dogfood_compare.RunSpec(provider="codex", profile="bare")
        config = dogfood_compare.RunConfig(
            cwd="/repo",
            prompt="Read files only and summarize.",
            mode="research",
            timeout_seconds=30,
            observe_timeout_ms=60_000,
            transcript_limit=200,
            result_max_bytes=100_000,
        )

        with tempfile.TemporaryDirectory() as tmpdir:
            summary = dogfood_compare.run_one(client, run, config, Path(tmpdir))
            run_dir = Path(tmpdir) / "runs" / "codex" / "bare"

            self.assertEqual(summary["provider"], "codex")
            self.assertEqual(summary["profile"], "bare")
            self.assertEqual(summary["status"], "succeeded")
            self.assertEqual(summary["agentId"], "agent_1")
            self.assertTrue((run_dir / "agent_spawn.json").exists())
            self.assertTrue((run_dir / "agent_observe.json").exists())
            self.assertTrue((run_dir / "task_transcript.json").exists())
            self.assertTrue((run_dir / "task_result.json").exists())

            transcript = json.loads((run_dir / "task_transcript.json").read_text())
            self.assertEqual(transcript["events"][0]["kind"], "provider_result")

        self.assertEqual(
            [name for name, _arguments in client.calls],
            ["agent_spawn", "agent_observe", "agent_result"],
        )
        spawn_args = client.calls[0][1]
        self.assertEqual(spawn_args["provider"], "codex")
        self.assertEqual(spawn_args["profile"], "bare")
        self.assertEqual(spawn_args["isolation"], "none")
        self.assertEqual(spawn_args["mode"], "research")
        result_args = client.calls[2][1]
        self.assertEqual(result_args["sections"], ["summary", "stdout", "stderr", "transcript"])

    def test_build_env_can_enable_strict_validation(self):
        env = dogfood_compare.build_env("/repo", strict_validation=True)

        self.assertEqual(env["AGENT_BRIDGE_WORKSPACES"], "/repo")
        self.assertEqual(env["AGENT_BRIDGE_STRICT_VALIDATION"], "true")


if __name__ == "__main__":
    unittest.main()
