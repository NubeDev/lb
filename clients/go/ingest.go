// The ingest surface — the durable write path + the read-back. Mirrors
// rust/role/gateway/src/routes/ingest.rs 1:1. The Producer field of a Sample is
// host-forced to the authenticated principal (un-spoofable), so callers may
// leave it empty here.

package lbclient

import (
	"context"
	"net/http"
	"net/url"
)

// Sample is the canonical Sample envelope (see crates/ingest/src/sample.rs).
// Producer is host-forced; Labels, QOS optional. Payload is any
// JSON-serializable value (typed as any to mirror the gateway's
// payload-agnostic envelope).
type Sample struct {
	Series   string         `json:"series"`
	Producer string         `json:"producer,omitempty"` // host-overridden; omit on the wire
	TS       uint64         `json:"ts"`
	Seq      uint64         `json:"seq"`
	Payload  any            `json:"payload"`
	Labels   map[string]any `json:"labels,omitempty"`
	QOS      string         `json:"qos,omitempty"` // "best-effort" (default) | "must-deliver"
}

// WriteSamplesReply is the POST /ingest reply.
type WriteSamplesReply struct {
	Accepted  uint64 `json:"accepted"`
	Committed uint64 `json:"committed"`
}

// LatestSampleReply is the GET /series/{s}/latest reply. Sample is the raw
// committed envelope, or nil when the series is empty (a JSON null).
type LatestSampleReply struct {
	Sample *Sample `json:"sample"`
}

// WriteSamples pushes samples to the durable ingest buffer. Returns
// {Accepted, Committed} — the staged count and the count drained to the
// committed series table on the same call (the gateway node carries the ingest
// path, so the write is visible to the next read).
func (c *Client) WriteSamples(ctx context.Context, samples []Sample) (*WriteSamplesReply, error) {
	body := struct {
		Samples []Sample `json:"samples"`
	}{Samples: samples}
	out := &WriteSamplesReply{}
	if err := c.requestJSON(ctx, http.MethodPost, "/ingest", body, out, false); err != nil {
		return nil, err
	}
	return out, nil
}

// LatestSample GETs /series/{series}/latest — the newest committed sample, or a
// nil Sample if the series has no samples yet. The simplest read-back proving
// the round-trip.
func (c *Client) LatestSample(ctx context.Context, series string) (*LatestSampleReply, error) {
	path := "/series/" + url.PathEscape(series) + "/latest"
	out := &LatestSampleReply{}
	if err := c.requestJSON(ctx, http.MethodGet, path, nil, out, false); err != nil {
		return nil, err
	}
	return out, nil
}
