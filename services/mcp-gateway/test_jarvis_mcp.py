"""Allow-list enforcement tests for the JARVIS MCP gateway.

Run from this directory with: python3 -m unittest -v
"""
import unittest

import jarvis_mcp
from jarvis_mcp import ALLOWED_VMIDS, TOOLS, Handler


class _FakeRuntime:
    """Minimal Runtime stand-in so the allowed path never touches Proxmox."""

    def __init__(self) -> None:
        self.calls: list[str] = []

    def pve_get(self, path: str) -> dict:
        self.calls.append(path)
        return {
            "vmid": 124, "name": "jarvis-core", "status": "running",
            "uptime": 1, "cpu": 0.0, "mem": 1, "maxmem": 2,
        }


def _handler() -> Handler:
    # Build a Handler without BaseHTTPRequestHandler.__init__ (which needs a socket).
    return Handler.__new__(Handler)


class AllowListTest(unittest.TestCase):
    def test_allowlist_is_the_jarvis_pool(self) -> None:
        # The Proxmox pool "JARVIS" currently holds only CT124 (core) and CT125 (voice).
        # 126 (mcp) and 127 (prometheus) are outside the pool on purpose.
        self.assertEqual(ALLOWED_VMIDS, {124, 125})

    def test_status_schema_enum_tracks_allowlist(self) -> None:
        status_tool = next(t for t in TOOLS if t["name"] == "proxmox.vm.status")
        enum = status_tool["inputSchema"]["properties"]["vmid"]["enum"]
        self.assertEqual(sorted(enum), sorted(ALLOWED_VMIDS))

    def test_status_rejects_vmid_outside_allowlist(self) -> None:
        handler = _handler()
        jarvis_mcp.RUNTIME = _FakeRuntime()
        for vmid in (0, 100, 126, 127, 999):
            with self.subTest(vmid=vmid):
                with self.assertRaises(ValueError):
                    handler._call_tool("proxmox.vm.status", {"vmid": vmid})
        # Rejection happens before any Proxmox request is issued.
        self.assertEqual(jarvis_mcp.RUNTIME.calls, [])

    def test_status_allows_vmid_inside_allowlist(self) -> None:
        handler = _handler()
        fake = _FakeRuntime()
        jarvis_mcp.RUNTIME = fake
        for vmid in ALLOWED_VMIDS:
            with self.subTest(vmid=vmid):
                result = handler._call_tool("proxmox.vm.status", {"vmid": vmid})
                self.assertFalse(result["isError"])
        self.assertEqual(len(fake.calls), len(ALLOWED_VMIDS))


if __name__ == "__main__":
    unittest.main()
