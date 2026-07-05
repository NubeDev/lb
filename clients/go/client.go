// Package lbclient — Client + the shared HTTP plumbing + the error type.
// The bearer is opaque to this library; see the package docstring.
//
// Uses only the Go standard library (net/http, crypto/hmac, encoding/json) so
// the client installs with `go get` and nothing else.

package lbclient

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
)

// APIError is a structured failure from the gateway (a non-2xx response),
// carrying the status + body verbatim so a caller can branch on "denied" vs
// "bad input" without us guessing. IsDenied covers the opaque 401|403|404
// statuses the gateway returns for missing-cap / cross-workspace /
// unknown-record (the contract never distinguishes them).
type APIError struct {
	Status int
	Body   string
	Path   string
}

func (e *APIError) Error() string {
	return fmt.Sprintf("gateway returned %d at %s: %s", e.Status, e.Path, e.Body)
}

// IsDenied reports whether the gateway's opaque deny path produced this error
// (missing cap, cross-workspace, or unknown record).
func (e *APIError) IsDenied() bool {
	return e.Status == 401 || e.Status == 403 || e.Status == 404
}

// LoginReply is the POST /login reply (see routes/login.rs::LoginReply).
type LoginReply struct {
	Token     string   `json:"token"`
	Principal string   `json:"principal"`
	Workspace string   `json:"workspace"`
	Caps      []string `json:"caps"`
}

// Client is a configured gateway client. The zero value is NOT usable —
// construct with New. Clone by value (only a string + URL + reusable
// *http.Client); replace the bearer with WithBearer.
type Client struct {
	BaseURL string
	bearer  string
	http    *http.Client
}

// New constructs a Client from a base URL (e.g. "http://127.0.0.1:8080") and a
// bearer credential — either an API key lbk_{ws}.{id}.{secret} or a JWT. Read
// the key from an env var in real code; do not hard-code it.
func New(baseURL, bearer string) *Client {
	return &Client{
		BaseURL: strings.TrimRight(baseURL, "/"),
		bearer:  bearer,
		http:    http.DefaultClient,
	}
}

// WithBearer returns a copy of c carrying the new bearer (used by Login; also
// useful for rotation).
func (c *Client) WithBearer(bearer string) *Client {
	return &Client{BaseURL: c.BaseURL, bearer: bearer, http: c.http}
}

// Login POSTs /login {user, workspace} — the dev-login path. Use for local-dev
// / admin scripts; for a long-lived producer, mint an API key once via the
// admin console (or POST /admin/apikeys) and use New with it. Returns a NEW
// *Client carrying the issued session token + the parsed reply.
func (c *Client) Login(ctx context.Context, user, workspace string) (*Client, *LoginReply, error) {
	body := map[string]string{"user": user, "workspace": workspace}
	var reply LoginReply
	if err := c.requestJSON(ctx, http.MethodPost, "/login", body, &reply, true); err != nil {
		return nil, nil, err
	}
	return c.WithBearer(reply.Token), &reply, nil
}

// request is the one HTTP plumbing function the verbs share. Carries the
// bearer (unless noBearer). in may be any JSON-serializable value; out, if
// non-nil, is filled from the response body. Use the typed verbs (WriteSamples,
// LatestSample, CallMCP, PostWebhook) rather than calling this directly.
func (c *Client) request(
	ctx context.Context,
	method, path string,
	in any,
	out any,
	noBearer bool,
	extraHeaders map[string]string,
	rawBody []byte,
) error {
	url := c.BaseURL + path
	var bodyReader io.Reader
	if rawBody != nil {
		bodyReader = bytes.NewReader(rawBody)
	} else if in != nil {
		buf, err := json.Marshal(in)
		if err != nil {
			return fmt.Errorf("lbclient: marshal body: %w", err)
		}
		bodyReader = bytes.NewReader(buf)
	}
	req, err := http.NewRequestWithContext(ctx, method, url, bodyReader)
	if err != nil {
		return fmt.Errorf("lbclient: build request: %w", err)
	}
	req.Header.Set("accept", "application/json")
	if rawBody != nil || in != nil {
		req.Header.Set("content-type", "application/json")
	}
	if !noBearer {
		req.Header.Set("authorization", "Bearer "+strings.TrimSpace(c.bearer))
	}
	for k, v := range extraHeaders {
		req.Header.Set(k, v)
	}
	resp, err := c.http.Do(req)
	if err != nil {
		return fmt.Errorf("lbclient: transport: %w", err)
	}
	defer resp.Body.Close()
	raw, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("lbclient: read body: %w", err)
	}
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return &APIError{Status: resp.StatusCode, Body: string(raw), Path: path}
	}
	if out != nil && len(raw) > 0 {
		if err := json.Unmarshal(raw, out); err != nil {
			return &APIError{Status: resp.StatusCode, Body: "invalid JSON: " + err.Error(), Path: path}
		}
	}
	return nil
}

// requestJSON is the common case: a JSON body in, a typed JSON body out.
func (c *Client) requestJSON(
	ctx context.Context,
	method, path string,
	in, out any,
	noBearer bool,
) error {
	return c.request(ctx, method, path, in, out, noBearer, nil, nil)
}
