"""`lb_client` — a thin external client for a Lazybones gateway node.

The five-method shape (mirrored across the four language clients under
`clients/`): construct a :class:`Client` with a base URL + a bearer, then call
:func:`write_samples` / :func:`latest_sample` / :func:`call_mcp` /
:func:`sign_webhook` / :func:`post_webhook`. The bearer is EITHER an API key
(``lbk_{ws}.{id}.{secret}``) OR a JWT from ``/login``; this library does not
branch on which — the gateway already splits on the ``lbk_`` prefix in one place
(``rust/role/gateway/src/session/authenticate.rs``).

See ``README.md`` for the auth + round-trip walkthrough.
"""

from lb_client.client import ApiError, Client, LoginReply
from lb_client.ingest import (
    LatestSampleReply,
    Sample,
    WriteSamplesReply,
    latest_sample,
    write_samples,
)
from lb_client.mcp import call_mcp
from lb_client.webhook import (
    WebhookAccepted,
    post_webhook,
    sign_webhook,
)

__all__ = [
    "ApiError",
    "Client",
    "LoginReply",
    "Sample",
    "WriteSamplesReply",
    "LatestSampleReply",
    "WebhookAccepted",
    "write_samples",
    "latest_sample",
    "call_mcp",
    "sign_webhook",
    "post_webhook",
]
