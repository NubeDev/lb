"""The ingest surface — the durable write path + the read-back. Mirrors
``rust/role/gateway/src/routes/ingest.rs`` 1:1. The ``producer`` field of a
:class:`Sample` is host-forced to the authenticated principal (un-spoofable),
so callers may leave it ``None`` here."""

from __future__ import annotations

from typing import Any, Optional, TypedDict

from lb_client.client import Client


class Sample(TypedDict, total=False):
    """The canonical ``Sample`` envelope (see ``crates/ingest/src/sample.rs``).
    ``producer`` is host-forced; ``labels``, ``qos`` optional."""

    series: str           # required
    ts: int               # required
    seq: int              # required (monotonic per (series, producer))
    payload: Any          # required
    producer: str         # host-overridden with the authenticated principal
    labels: dict[str, Any]
    qos: str              # "best-effort" (default) | "must-deliver"


class WriteSamplesReply(TypedDict):
    """``POST /ingest`` reply."""

    accepted: int
    committed: int


class LatestSampleReply(TypedDict):
    """``GET /series/{s}/latest`` reply — ``sample`` is the raw committed
    envelope, or ``None`` when the series is empty."""

    sample: Optional[dict[str, Any]]


def write_samples(client: Client, samples: list[Sample]) -> WriteSamplesReply:
    """Push ``samples`` to the durable ingest buffer. Returns
    ``{accepted, committed}`` — the staged count and the count drained to the
    committed ``series`` table on the same call (the gateway node carries the
    ingest path, so the write is visible to the next read)."""
    return client._request_json("POST", "/ingest", body={"samples": samples})


def latest_sample(client: Client, series: str) -> LatestSampleReply:
    """``GET /series/{series}/latest`` — the newest committed sample, or
    ``None`` if the series has no samples yet. The simplest read-back proving
    the round-trip."""
    from urllib.parse import quote

    path = f"/series/{quote(series, safe='')}/latest"
    return client._request_json("GET", path)
