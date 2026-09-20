import base64
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from urllib.error import URLError
from urllib.parse import parse_qs, urlparse


SCRIPT_PATH = (
    Path(__file__).resolve().parents[1] / "infra" / "seven-days" / "duckdns.py"
)
SPEC = importlib.util.spec_from_file_location("seven_days_duckdns", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
DUCKDNS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DUCKDNS)


class FakeResponse:
    def __init__(self, payload: bytes):
        self.payload = payload

    def __enter__(self):
        return self

    def __exit__(self, _type, _value, _traceback):
        return False

    def read(self) -> bytes:
        return self.payload


class FakeOpener:
    def __init__(self, fail_duckdns: bool = False, external_ip: bytes = b"203.0.113.42"):
        self.fail_duckdns = fail_duckdns
        self.external_ip = external_ip
        self.requests = []

    def __call__(self, request, timeout):
        self.requests.append((request, timeout))
        url = request.full_url
        if url.endswith("/external-ip"):
            return FakeResponse(self.external_ip)
        if url.endswith("/service-accounts/default/token"):
            return FakeResponse(json.dumps({"access_token": "access-token"}).encode())
        if url.endswith("/versions/latest:access"):
            payload = base64.b64encode(b"duck-secret").decode()
            return FakeResponse(json.dumps({"payload": {"data": payload}}).encode())
        if url.startswith(DUCKDNS.DUCKDNS_UPDATE_URL):
            if self.fail_duckdns:
                raise URLError("request URL contained duck-secret")
            return FakeResponse(b"OK")
        raise AssertionError(f"unexpected request: {url}")


class DuckDnsUpdateTests(unittest.TestCase):
    def config_path(self, config=None):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        path = Path(temporary.name) / "config.json"
        path.write_text(
            json.dumps(
                config
                or {
                    "project_id": "test-project",
                    "subdomain": "zunda-7dtd",
                    "secret_id": "SEVEN_DAYS_DUCKDNS_TOKEN",
                }
            ),
            encoding="utf-8",
        )
        return path

    def test_updates_current_external_ip_with_secret_manager_token(self):
        opener = FakeOpener()

        DUCKDNS.update(self.config_path(), opener)

        update_request = opener.requests[-1][0]
        query = parse_qs(urlparse(update_request.full_url).query)
        self.assertEqual(query["domains"], ["zunda-7dtd"])
        self.assertEqual(query["ip"], ["203.0.113.42"])
        self.assertEqual(query["token"], ["duck-secret"])
        secret_request = opener.requests[-2][0]
        self.assertEqual(
            secret_request.get_header("Authorization"), "Bearer access-token"
        )

    def test_request_error_does_not_expose_secret_or_url(self):
        opener = FakeOpener(fail_duckdns=True)

        with self.assertRaises(DUCKDNS.DuckDnsUpdateError) as raised:
            DUCKDNS.update(self.config_path(), opener)

        message = str(raised.exception)
        self.assertNotIn("duck-secret", message)
        self.assertNotIn(DUCKDNS.DUCKDNS_UPDATE_URL, message)
        self.assertIn("URLError", message)

    def test_rejects_invalid_subdomain_before_network_access(self):
        opener = FakeOpener()
        path = self.config_path(
            {
                "project_id": "test-project",
                "subdomain": "https://example.com",
                "secret_id": "SEVEN_DAYS_DUCKDNS_TOKEN",
            }
        )

        with self.assertRaisesRegex(
            DUCKDNS.DuckDnsUpdateError, "invalid subdomain"
        ):
            DUCKDNS.update(path, opener)

        self.assertEqual(opener.requests, [])

    def test_rejects_invalid_external_ip_before_reading_secret(self):
        opener = FakeOpener(external_ip=b"not-an-ip")

        with self.assertRaisesRegex(
            DUCKDNS.DuckDnsUpdateError, "valid external IPv4"
        ):
            DUCKDNS.update(self.config_path(), opener)

        self.assertEqual(len(opener.requests), 1)


if __name__ == "__main__":
    unittest.main()
