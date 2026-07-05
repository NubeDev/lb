// roundtrip.go — login → write a Sample → read it back, against a real
// `make cloud` node.
//
// Run with:
//   make cloud                       # terminal 1: boot 127.0.0.1:8080
//   cd clients/go
//   LB_URL=http://127.0.0.1:8080 LB_USER=ada LB_WORKSPACE=acme go run ./cmd/roundtrip
//   # or with an API key:
//   LB_KEY=lbk_acme.k7f3a.ABCDEF23 go run ./cmd/roundtrip
package main

import (
	"context"
	"flag"
	"log"
	"os"
	"time"

	"github.com/lazybones/lb/clients/go"
)

func main() {
	urlFlag := flag.String("url", envOr("LB_URL", "http://127.0.0.1:8080"), "gateway base URL")
	user := flag.String("user", envOr("LB_USER", "ada"), "login user (dev-login)")
	ws := flag.String("workspace", envOr("LB_WORKSPACE", "acme"), "login workspace")
	flag.Parse()

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	// 1. Authenticate — API key (preferred) or dev-login.
	client := lbclient.New(*urlFlag, "placeholder")
	if key, ok := os.LookupEnv("LB_KEY"); ok {
		client = client.WithBearer(key)
	} else {
		c, reply, err := client.Login(ctx, *user, *ws)
		if err != nil {
			log.Fatalf("login: %v", err)
		}
		log.Printf("logged in as %s in %s", reply.Principal, reply.Workspace)
		client = c
	}

	// 2. Push one Sample. Producer is host-forced, so leave it empty.
	written, err := client.WriteSamples(ctx, []lbclient.Sample{{
		Series:  "demo.cpu_temp",
		TS:      uint64(time.Now().UnixMilli()),
		Seq:     1,
		Payload: 61.4,
		Labels:  map[string]any{"host": "pi-7"},
	}})
	if err != nil {
		log.Fatalf("write: %v", err)
	}
	log.Printf("accepted=%d committed=%d", written.Accepted, written.Committed)

	// 3. Read the newest value back — the round-trip.
	latest, err := client.LatestSample(ctx, "demo.cpu_temp")
	if err != nil {
		log.Fatalf("latest: %v", err)
	}
	log.Printf("latest sample: %+v", latest.Sample)

	// 4. The universal MCP bridge: every other verb is one call away.
	list, err := client.CallMCP(ctx, "series.list", map[string]any{})
	if err != nil {
		log.Printf("series.list: %v (continuing)", err)
	} else {
		log.Printf("series in workspace: %v", list)
	}
}

func envOr(key, def string) string {
	if v, ok := os.LookupEnv(key); ok {
		return v
	}
	return def
}
