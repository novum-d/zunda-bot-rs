#!/usr/bin/env python3
"""Update the 7DTD DuckDNS record without exposing its token."""

from __future__ import annotations

import base64
import binascii
import ipaddress
import json
import re
import sys
from pathlib import Path
from typing import Any, Callable
from urllib.parse import quote, urlencode
from urllib.request import Request, urlopen

CONFIG_PATH = Path("/etc/seven-days-duckdns.json")
METADATA_ROOT = "http://metadata.google.internal/computeMetadata/v1"
DUCKDNS_UPDATE_URL = "https://www.duckdns.org/update"
REQUEST_TIMEOUT_SECONDS = 10
SUBDOMAIN_PATTERN = re.compile(r"^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$")
SECRET_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]+$")

Opener = Callable[..., Any]


class DuckDnsUpdateError(RuntimeError):
    """Safe-to-log error that never contains credentials or request URLs."""


def request_bytes(
    url: str,
    *,
    headers: dict[str, str] | None = None,
    opener: Opener = urlopen,
) -> bytes:
    request = Request(url, headers=headers or {})
    try:
        with opener(request, timeout=REQUEST_TIMEOUT_SECONDS) as response:
            return response.read()
    except Exception as error:
        raise DuckDnsUpdateError(
            f"request failed ({type(error).__name__})"
        ) from None


def parse_json(payload: bytes, description: str) -> dict[str, Any]:
    try:
        value = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError):
        raise DuckDnsUpdateError(f"{description} returned invalid JSON") from None
    if not isinstance(value, dict):
        raise DuckDnsUpdateError(f"{description} returned an invalid object")
    return value


def required_string(value: dict[str, Any], key: str, description: str) -> str:
    field = value.get(key)
    if not isinstance(field, str) or not field:
        raise DuckDnsUpdateError(f"{description} did not include {key}")
    return field


def load_config(path: Path) -> tuple[str, str, str]:
    try:
        config = parse_json(path.read_bytes(), "DuckDNS config")
    except OSError as error:
        raise DuckDnsUpdateError(
            f"DuckDNS config could not be read ({type(error).__name__})"
        ) from None

    project_id = required_string(config, "project_id", "DuckDNS config")
    subdomain = required_string(config, "subdomain", "DuckDNS config")
    secret_id = required_string(config, "secret_id", "DuckDNS config")
    if not SUBDOMAIN_PATTERN.fullmatch(subdomain):
        raise DuckDnsUpdateError("DuckDNS config contains an invalid subdomain")
    if not SECRET_ID_PATTERN.fullmatch(secret_id):
        raise DuckDnsUpdateError("DuckDNS config contains an invalid secret ID")
    return project_id, subdomain, secret_id


def metadata(path: str, opener: Opener) -> bytes:
    return request_bytes(
        f"{METADATA_ROOT}/{path}",
        headers={"Metadata-Flavor": "Google"},
        opener=opener,
    )


def access_secret(project_id: str, secret_id: str, opener: Opener) -> str:
    token_response = parse_json(
        metadata("instance/service-accounts/default/token", opener),
        "metadata token endpoint",
    )
    access_token = required_string(
        token_response, "access_token", "metadata token endpoint"
    )
    secret_url = (
        "https://secretmanager.googleapis.com/v1/projects/"
        f"{quote(project_id, safe='')}/secrets/{quote(secret_id, safe='')}"
        "/versions/latest:access"
    )
    secret_response = parse_json(
        request_bytes(
            secret_url,
            headers={"Authorization": f"Bearer {access_token}"},
            opener=opener,
        ),
        "Secret Manager",
    )
    payload = secret_response.get("payload")
    if not isinstance(payload, dict):
        raise DuckDnsUpdateError("Secret Manager did not include a payload")
    encoded_secret = required_string(payload, "data", "Secret Manager payload")
    try:
        secret = base64.b64decode(encoded_secret, validate=True).decode("utf-8").strip()
    except (binascii.Error, UnicodeDecodeError):
        raise DuckDnsUpdateError("Secret Manager returned an invalid payload") from None
    if not secret:
        raise DuckDnsUpdateError("Secret Manager returned an empty payload")
    return secret


def update(path: Path = CONFIG_PATH, opener: Opener = urlopen) -> None:
    project_id, subdomain, secret_id = load_config(path)
    external_ip = metadata(
        "instance/network-interfaces/0/access-configs/0/external-ip", opener
    ).decode("ascii").strip()
    try:
        ipaddress.IPv4Address(external_ip)
    except ipaddress.AddressValueError:
        raise DuckDnsUpdateError("metadata did not include a valid external IPv4") from None
    token = access_secret(project_id, secret_id, opener)
    query = urlencode({"domains": subdomain, "token": token, "ip": external_ip})
    response = request_bytes(f"{DUCKDNS_UPDATE_URL}?{query}", opener=opener)
    if response.strip() != b"OK":
        raise DuckDnsUpdateError("DuckDNS rejected the update")


def main() -> int:
    try:
        update()
    except DuckDnsUpdateError as error:
        print(f"seven-days DuckDNS update failed: {error}", file=sys.stderr)
        return 1
    print("seven-days DuckDNS update succeeded")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
