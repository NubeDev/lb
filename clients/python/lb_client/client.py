"""The ``Client`` — base URL + bearer credential + the one HTTP plumbing
function the other verbs share. The bearer is opaque to this library; see the
package docstring for why.

Uses only the Python standard library (``urllib`` for HTTP, ``json`` for the
body) so the client installs with zero deps. Python 3.9+.
"""

from __future__ import annotations

import json as _json
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from typing import Any, Mapping, Optional, TypedDict


class ApiError(Exception):
    """A structured failure from the gateway (a non-2xx response). Carries the
    status + body verbatim so the caller can branch on "denied" vs "bad input"
    without us guessing. ``is_denied()`` covers the opaque ``401|403|404``
    statuses the gateway returns for missing-cap / cross-workspace /
    unknown-record (the contract never distinguishes them)."""

    def __init__(self, status: int, body: str, path: str) -> None:
        super().__init__(f"gateway returned {status} at {path}: {body}")
        self.status = status
        self.body = body
        self.path = path

    def is_denied(self) -> bool:
        return self.status in (401, 403, 404)


class LoginReply(TypedDict):
    """The ``POST /login`` reply (see ``routes/login.rs::LoginReply``)."""

    token: str
    principal: str
    workspace: str
    caps: list[str]


@dataclass
class Client:
    """A configured gateway client. Cheap to copy (only a string + URL).

    Construct with a base URL (e.g. ``http://127.0.0.1:8080``) and a bearer
    credential — either an API key ``lbk_{ws}.{id}.{secret}`` or a JWT. **Read
    the key from an env var in real code; do not hard-code it.**"""

    base_url: str
    bearer: str

    def __post_init__(self) -> None:
        self.base_url = self.base_url.rstrip("/")

    def with_bearer(self, bearer: str) -> "Client":
        """Return a new :class:`Client` with the bearer replaced (used by
        :meth:`login`; also useful for rotation)."""
        return Client(self.base_url, bearer)

    def login(self, user: str, workspace: str) -> tuple["Client", LoginReply]:
        """``POST /login {user, workspace}`` — the dev-login path. Use for
        local-dev / admin scripts; for a long-lived producer, mint an API key
        once via the admin console (or ``POST /admin/apikeys``) and construct
        :class:`Client` with it. Returns a NEW ``Client`` carrying the issued
        session token + the parsed reply."""
        reply: LoginReply = self._request_json(
            "POST", "/login", body={"user": user, "workspace": workspace}, no_bearer=True,
        )
        return self.with_bearer(reply["token"]), reply

    # --- plumbing ---------------------------------------------------------

    def request(
        self,
        method: str,
        path: str,
        *,
        body: Any = None,
        raw_body: Optional[bytes] = None,
        headers: Optional[Mapping[str, str]] = None,
        no_bearer: bool = False,
    ) -> tuple[int, bytes]:
        """Run one HTTP request; return ``(status, raw_body_bytes)``. Adds the
        bearer (unless ``no_bearer``). Use the typed verbs in ``ingest.py`` /
        ``mcp.py`` / ``webhook.py`` rather than calling this directly.

        ``body`` (JSON-serializable) and ``raw_body`` (bytes) are mutually
        exclusive; the webhook path uses ``raw_body`` so it can sign the exact
        bytes it sends."""
        url = f"{self.base_url}{path}"
        hdrs: dict[str, str] = {"accept": "application/json"}
        if headers:
            hdrs.update(headers)
        if not no_bearer:
            hdrs["authorization"] = f"Bearer {self.bearer.strip()}"
        data: Optional[bytes] = None
        if raw_body is not None:
            hdrs.setdefault("content-type", "application/json")
            data = raw_body
        elif body is not None:
            hdrs["content-type"] = "application/json"
            data = _json.dumps(body).encode("utf-8")
        req = urllib.request.Request(url, data=data, method=method, headers=hdrs)
        try:
            with urllib.request.urlopen(req) as resp:  # noqa: S310 — caller-supplied URL
                return resp.status, resp.read()
        except urllib.error.HTTPError as e:
            return e.code, e.read()

    def _request_json(
        self,
        method: str,
        path: str,
        *,
        body: Any = None,
        headers: Optional[Mapping[str, str]] = None,
        no_bearer: bool = False,
    ) -> Any:
        status, raw = self.request(
            method, path, body=body, headers=headers, no_bearer=no_bearer,
        )
        if not (200 <= status < 300):
            raise ApiError(status, raw.decode("utf-8", "replace"), path)
        if not raw:
            return None
        try:
            return _json.loads(raw)
        except _json.JSONDecodeError as e:
            raise ApiError(status, f"invalid JSON: {e}", path) from e
