"""The webhook helper — the **third-party caller path**. A service the admin
has shared a webhook secret with signs the raw body and POSTs to
``/hooks/{ws}/{id}``. The gateway verifies the HMAC over the **exact received
bytes** (see ``routes/webhook.rs``), so this helper takes ``bytes``, never a
``str`` — HMAC over a re-serialized body is the single most common
webhook-integration bug (pinned in
``webhook_routes_test.rs::signature_mode_body_tamper_breaks_signature``)."""

from __future__ import annotations

import hashlib
import hmac
from typing import Mapping, TypedDict
from urllib.parse import quote

from lb_client.client import ApiError, Client


class WebhookAccepted(TypedDict):
    """``POST /hooks/{ws}/{id}`` reply (see ``routes/webhook.rs::Accepted``)."""

    id: str
    series: str
    seq: int


def sign_webhook(secret: bytes, body: bytes) -> str:
    """Sign ``body`` with ``secret`` (the shared secret the admin got at
    webhook create). Returns the value to send in the admin-picked header
    (default ``X-Signature``), formatted as ``sha256=<64 hex>`` — exactly what
    the gateway's ``signature`` mode expects.

    **Body must be the raw bytes you POST** — sign-then-reformat breaks the
    signature."""
    mac = hmac.new(secret, body, hashlib.sha256).hexdigest()
    return f"sha256={mac}"


def post_webhook(
    client: Client,
    ws: str,
    id: str,
    headers: Mapping[str, str],
    body: bytes,
) -> WebhookAccepted:
    """``POST /hooks/{ws}/{id}`` with caller-supplied headers. For ``signature``
    mode, pass ``{"X-Signature": sign_webhook(secret, body)}`` (or the
    admin-picked header name). For ``bearer`` mode, pass
    ``{"Authorization": "Bearer lbk_…"}``. The :class:`Client`'s own bearer is
    NOT applied here — the inbound webhook route is the one gateway route that
    takes no session token."""
    path = f"/hooks/{quote(ws, safe='')}/{quote(id, safe='')}"
    # `no_bearer=True` so the client's bearer is not attached by the plumbing;
    # `raw_body=body` so the bytes we sign are the bytes we send.
    status, raw = client.request(
        "POST", path, raw_body=body, headers={**headers}, no_bearer=True,
    )
    if not (200 <= status < 300):
        raise ApiError(status, raw.decode("utf-8", "replace"), path)
    from json import loads

    return loads(raw)
