// The webhook helper — the third-party caller path. A service the admin has
// shared a webhook secret with signs the raw body and POSTs to
// /hooks/{ws}/{id}. The gateway verifies the HMAC over the EXACT received
// bytes (see routes/webhook.rs), so this helper takes []byte, never a string —
// HMAC over a re-serialized body is the single most common webhook-integration
// bug (pinned in webhook_routes_test.rs::signature_mode_body_tamper_breaks_signature).

package lbclient

import (
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"net/http"
	"net/url"
)

// WebhookAccepted is the POST /hooks/{ws}/{id} reply (see routes/webhook.rs::Accepted).
type WebhookAccepted struct {
	ID     string `json:"id"`
	Series string `json:"series"`
	Seq    uint64 `json:"seq"`
}

// SignWebhook signs body with secret (the shared secret the admin got at
// webhook create). Returns the value to send in the admin-picked header
// (default "X-Signature"), formatted as "sha256=<64 hex>" — exactly what the
// gateway's signature mode expects.
//
// Body must be the raw bytes you POST — sign-then-reformat breaks the signature.
func SignWebhook(secret, body []byte) string {
	mac := hmac.New(sha256.New, secret)
	mac.Write(body)
	return "sha256=" + hex.EncodeToString(mac.Sum(nil))
}

// PostWebhook POSTs /hooks/{ws}/{id} with caller-supplied headers. For
// signature mode, pass {"X-Signature": SignWebhook(secret, body)} (or the
// admin-picked header name). For bearer mode, pass
// {"Authorization": "Bearer lbk_…"}. The Client's own bearer is NOT applied
// here — the inbound webhook route is the one gateway route that takes no
// session token.
func (c *Client) PostWebhook(
	ctx context.Context,
	ws, id string,
	headers map[string]string,
	body []byte,
) (*WebhookAccepted, error) {
	path := "/hooks/" + url.PathEscape(ws) + "/" + url.PathEscape(id)
	out := &WebhookAccepted{}
	// noBearer=true: this is the inbound webhook route — the caller's headers
	// are the only credential on the wire.
	// rawBody=body: the bytes we sign are the bytes we send.
	if err := c.request(ctx, http.MethodPost, path, nil, out, true, headers, body); err != nil {
		return nil, err
	}
	return out, nil
}
