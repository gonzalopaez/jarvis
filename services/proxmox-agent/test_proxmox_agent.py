import io, json, tempfile, unittest
from pathlib import Path
from unittest.mock import patch
from proxmox_agent import CAPABILITIES, CORE_TIMEOUT_SECONDS, ProxmoxAgent, mcp_loop

class FakeResponse:
    def __init__(self, body): self.body=body
    def __enter__(self): return self
    def __exit__(self,*_): return False
    def read(self,_): return self.body

class ProxmoxAgentTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(); self.token=Path(self.temp.name)/"token"; self.token.write_text("t"*32); self.agent=ProxmoxAgent("https://core",str(self.token))
    def tearDown(self): self.temp.cleanup()
    def test_all_tier_3_capabilities_are_exposed_and_nothing_else(self):
        output=io.StringIO()
        with patch("sys.stdin",io.StringIO(json.dumps({"id":1,"method":"tools/list"})+"\n")),patch("sys.stdout",output): mcp_loop(self.agent)
        self.assertEqual({x["name"] for x in json.loads(output.getvalue())["result"]["tools"]},CAPABILITIES)
    @patch("urllib.request.urlopen")
    def test_destroy_is_only_proposed_to_core_with_explicit_timeout(self,urlopen):
        urlopen.return_value=FakeResponse(json.dumps({"status":"authorization_required","error":{"code":"AUTHORIZATION_REQUIRED"}}).encode()); result=self.agent.propose("proxmox.vm.destroy","vm-104",{},"session-1"); payload=json.loads(urlopen.call_args.args[0].data); self.assertEqual(payload["kind"],"action"); self.assertNotIn("authorization",payload); self.assertEqual(urlopen.call_args.kwargs["timeout"],CORE_TIMEOUT_SECONDS); self.assertEqual(result["core_response"]["status"],"authorization_required")
    @patch("urllib.request.urlopen")
    def test_non_allowlisted_capability_never_reaches_network(self,urlopen):
        with self.assertRaises(ValueError): self.agent.propose("proxmox.vm.start","vm-104",{},"session-1")
        urlopen.assert_not_called()
    @patch("urllib.request.urlopen")
    def test_invalid_target_never_reaches_network(self,urlopen):
        with self.assertRaises(ValueError): self.agent.propose("proxmox.ct.destroy","CT 120",{},"session-1")
        urlopen.assert_not_called()

if __name__=="__main__": unittest.main(verbosity=2)
