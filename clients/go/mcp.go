// POST /mcp/call {tool, args} — the universal host-mediated bridge. Every
// platform verb that isn't wrapped by name in this library is reachable from
// here without a library update (see docs/skills/ingest-series/SKILL.md for the
// verb table). Re-checks the workspace + mcp:<tool>:call capability.

package lbclient

import (
	"context"
	"net/http"
)

// CallMCP calls tool with args over the bridge. args may be any
// JSON-serializable value (pass nil for a no-arg tool). Returns the tool's raw
// JSON output (decoded into a generic any — typically map[string]any).
func (c *Client) CallMCP(ctx context.Context, tool string, args any) (any, error) {
	if args == nil {
		args = map[string]any{}
	}
	body := struct {
		Tool string `json:"tool"`
		Args any    `json:"args"`
	}{Tool: tool, Args: args}
	var out any
	if err := c.requestJSON(ctx, http.MethodPost, "/mcp/call", body, &out, false); err != nil {
		return nil, err
	}
	return out, nil
}
