import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from wazuh_agent import AgentConfig, AlertStore, LLM_TIMEOUT_SECONDS, MAX_CONTEXT_BYTES, WazuhAgent, mcp_loop, normalize_alert


class FakeHttp:
    def __init__(self, verdict):
        self.verdict = verdict
        self.calls = []

    def post(self, url, token, payload, timeout):
        self.calls.append((url, token, payload, timeout))
        if url.endswith("/v1/chat/completions"):
            return {"choices": [{"message": {"content": json.dumps(self.verdict)}}]}
        return {"status": "authorization_required", "error": {"code": "AUTHORIZATION_REQUIRED"}}


class WazuhAgentTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        root = Path(self.temp.name)
        (root / "alerts.json").write_text(json.dumps({"id": "1", "rule": {"level": 13, "description": "attack"}, "agent": {"name": "host-01"}}) + "\n")
        (root / "litellm-token").write_text("l" * 26)
        (root / "core-token").write_text("c" * 32)
        (root / "relay-token").write_text("r" * 32)
        self.config = AgentConfig(
            str(root / "alerts.json"),
            str(root / "relay-token"),
            "http://litellm",
            str(root / "litellm-token"),
            "https://core",
            str(root / "core-token"),
            str(Path(__file__).resolve().parents[2] / "contracts/api/security-verdict.v1.schema.json"),
            "127.0.0.1",
            5515,
        )

    def tearDown(self):
        self.temp.cleanup()

    def test_read_alerts_is_normalized_and_read_only(self):
        alerts = AlertStore(self.config.alerts_path).read()
        self.assertEqual(alerts[0]["host"], "host-01")
        self.assertEqual(alerts[0]["severity"], "critical")

    def test_canonical_event_preserves_mitre_and_original_timestamp(self):
        fixture = json.loads((Path(__file__).parent / "fixtures/mitre_chain.json").read_text())
        event = normalize_alert(fixture)["wazuh"]
        self.assertEqual(event["timestamp"], "2026-01-01T10:06:00Z")
        self.assertEqual(event["timestamp_ms"], 1767261960000)
        self.assertEqual([item["id"] for item in event["mitre"]], ["T1078", "T1059.001", "T1105"])
        self.assertEqual(event["entities"]["src_user"], "test-user")
        self.assertIsNone(event["entities"]["dst_user"])

    def test_incomplete_event_does_not_invent_identifiers_or_host(self):
        fixture = json.loads((Path(__file__).parent / "fixtures/incomplete_alert.json").read_text())
        normalized = normalize_alert(fixture)
        self.assertIsNone(normalized["id"])
        self.assertIsNone(normalized["agent_id"])
        self.assertIsNone(normalized["host"])
        self.assertIsNone(normalized["timestamp"])
        self.assertIsNone(normalized["wazuh"]["mitre"])

    def test_mitre_arrays_do_not_fabricate_missing_labels(self):
        fixture = json.loads((Path(__file__).parent / "fixtures/mitre_multiple_arrays.json").read_text())
        mitre = normalize_alert(fixture)["wazuh"]["mitre"]
        self.assertEqual(mitre[1], {"id": "T1105", "tactic": None, "technique": "Ingress Tool Transfer"})

    def test_l2_triage_has_explicit_timeout_and_bounded_context(self):
        verdict = {"verdict": "AMENAZA_REAL_ALTA", "confidence": "ALTA", "justification": "evidence", "proposed_actions": []}
        http = FakeHttp(verdict)
        agent = WazuhAgent(self.config, http)
        agent.triage({"rule": {"level": 13}, "agent": {"name": "host-01"}})
        _, _, payload, timeout = http.calls[0]
        self.assertEqual(payload["model"], "jarvis-soc-l2")
        self.assertEqual(timeout, LLM_TIMEOUT_SECONDS)
        self.assertLessEqual(len(payload["messages"][1]["content"].encode()), MAX_CONTEXT_BYTES)

    def test_l1_triage_uses_scoped_alias(self):
        verdict = {"verdict": "FALSO_POSITIVO", "confidence": "ALTA", "justification": "evidence"}
        http = FakeHttp(verdict)
        WazuhAgent(self.config, http).triage({"rule": {"level": 8}, "agent": {"name": "host-01"}})
        self.assertEqual(http.calls[0][2]["model"], "jarvis-soc-l1")

    def test_proposal_reaches_core_as_action_and_is_not_executed_by_agent(self):
        verdict = {"verdict": "AMENAZA_REAL_ALTA", "confidence": "ALTA", "justification": "evidence", "proposed_actions": [{"capability": "security.host.isolate", "target": "host-01", "parameters": {"reason": "test"}}]}
        http = FakeHttp(verdict)
        result = WazuhAgent(self.config, http).propose(verdict, "session-test")
        _, _, payload, _ = http.calls[0]
        self.assertEqual(payload["kind"], "action")
        self.assertEqual(payload["action"]["capability"], "security.host.isolate")
        self.assertEqual(result[0]["core_response"]["status"], "authorization_required")
        self.assertEqual(len(http.calls), 1)

    def test_non_allowlisted_proposal_is_rejected_before_network(self):
        verdict = {"verdict": "AMENAZA_REAL_ALTA", "confidence": "ALTA", "justification": "evidence", "proposed_actions": [{"capability": "shell.execute", "target": "host-01", "parameters": {}}]}
        http = FakeHttp(verdict)
        with self.assertRaises(ValueError):
            WazuhAgent(self.config, http).propose(verdict, "session-test")
        self.assertEqual(http.calls, [])

    def test_non_contract_target_is_rejected_before_network(self):
        verdict = {"verdict": "AMENAZA_REAL_ALTA", "confidence": "ALTA", "justification": "evidence", "proposed_actions": [{"capability": "security.host.isolate", "target": "Host Uppercase", "parameters": {}}]}
        http = FakeHttp(verdict)
        with self.assertRaises(ValueError):
            WazuhAgent(self.config, http).propose(verdict, "session-test")
        self.assertEqual(http.calls, [])

    def test_mcp_lists_only_read_and_triage_tools(self):
        requests = "\n".join([
            json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize"}),
            json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        ]) + "\n"
        output = io.StringIO()
        with patch.object(sys, "stdin", io.StringIO(requests)), patch.object(sys, "stdout", output):
            mcp_loop(WazuhAgent(self.config, FakeHttp({})))
        responses = [json.loads(line) for line in output.getvalue().splitlines()]
        names = [tool["name"] for tool in responses[1]["result"]["tools"]]
        self.assertEqual(names, ["wazuh.alerts.read", "wazuh.alert.triage"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
