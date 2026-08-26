#!/usr/bin/env bash
# THE INBOX/OUTBOX LOOP, END TO END, ON REAL SOCKETS.
#
# lb's own OUTBOX emails a file to a mailbox; lb's own MAIL SOURCE polls that mailbox back over
# IMAP, decodes the attachment into series samples, and shows the arrival in the lb inbox. Nothing
# here is faked: a real SMTP server receives the message and a real IMAP server serves it (GreenMail
# in Docker), and every lb-side step goes through the production verbs over `POST /mcp/call`.
#
#   asset (NEM12 CSV) ─▶ outbox.enqueue{target:"email", assetId} ─▶ relay ─▶ SMTP:3025
#                                                                              │
#                                    GreenMail mailbox alerts@nube-io.com ◀────┘
#                                                 │
#                     mail reactor ─▶ IMAP:3143 ─▶ import ─▶ assets + series + inbox item
#
# Prereqs
#   * Docker (for GreenMail — `docker run` is done for you).
#   * A RUNNING node whose OUTBOX transport points at GreenMail. Start it with:
#       LB_MAIL_KIND=smtp LB_MAIL_HOST=127.0.0.1 LB_MAIL_PORT=3025 \
#       LB_MAIL_TLS=none LB_MAIL_AUTH=none LB_MAIL_FROM='Meter Data <data@example.com>' \
#       make cloud
#     (the mail SOURCE half needs no boot config at all — it is a record.)
#   * `curl` + `python3`.
#
# Usage:  bash docs/testing/inbox-outbox/mail-source-demo.sh [GATEWAY_URL] [EMAIL] [PASSWORD]
#
# Idempotent: stable ids everywhere. Re-running re-sends the mail under a NEW message id (so it
# imports again) — and the series will NOT grow, because a sample's dedup key is derived from its
# timestamp. That convergence is half the point of the demo.
set -euo pipefail

GW="${1:-http://127.0.0.1:8099}"
EMAIL="${2:-test@acme.local}"
PASSWORD="${3:-dev-admin-pw}"
# The real four-channel export, from lb-ingest's own test fixtures — so the demo is self-contained
# and always decodes the same bytes the decoder tests assert against.
CSV="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)/rust/crates/ingest/tests/fixtures/nem12-4-channel.csv"
MAILBOX="alerts@nube-io.com"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

command -v docker >/dev/null || { echo "needs docker (for the GreenMail SMTP+IMAP server)"; exit 1; }
[ -f "$CSV" ] || { echo "missing the NEM12 fixture at $CSV"; exit 1; }

say() { printf '\n\033[1m▸ %s\033[0m\n' "$*"; }

# ---------------------------------------------------------------------------------------------
say "1/8  a real mail server (GreenMail: SMTP 3025, IMAP 3143)"
if ! docker ps --format '{{.Names}}' | grep -qx lb-greenmail; then
  docker rm -f lb-greenmail >/dev/null 2>&1 || true
  docker run -d --name lb-greenmail -p 3025:3025 -p 3143:3143 \
    -e GREENMAIL_OPTS='-Dgreenmail.setup.test.smtp -Dgreenmail.setup.test.imap -Dgreenmail.hostname=0.0.0.0 -Dgreenmail.auth.disabled' \
    greenmail/standalone:2.1.9 >/dev/null
  # Probe the PORT, not the log: GreenMail only prints "Started imap" at DEBUG, so a log grep here
  # waits for ever on a server that is already up.
  for _ in $(seq 1 60); do
    (exec 3<>/dev/tcp/127.0.0.1/3143) 2>/dev/null && break
    sleep 1
  done
fi
(exec 3<>/dev/tcp/127.0.0.1/3143) 2>/dev/null || { echo "greenmail IMAP (3143) never came up"; exit 1; }
echo "   greenmail up (auth disabled: the mailbox is created on first use, password = the address)"

# ---------------------------------------------------------------------------------------------
say "2/8  sign in to the node"
TOKEN=$(curl -fsS -X POST "$GW/auth/login" -H 'content-type: application/json' \
  -d "$(python3 -c 'import json,sys; print(json.dumps({"email":sys.argv[1],"password":sys.argv[2]}))' "$EMAIL" "$PASSWORD")" \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["token"])')
echo "   token acquired (${#TOKEN} chars)"

mcp() { # mcp <tool> <args-file>
  python3 -c 'import json,sys; print(json.dumps({"tool":sys.argv[1],"args":json.load(open(sys.argv[2]))}))' "$1" "$2" \
  | curl -fsS -X POST "$GW/mcp/call" -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' -d @-
}

# ---------------------------------------------------------------------------------------------
say "3/8  the meter export becomes a workspace asset"
python3 - "$CSV" > "$TMP/asset.json" <<'PY'
import base64, json, sys
print(json.dumps({"id":"nem12-demo-export","mime":"text/csv",
                  "bytes":base64.b64encode(open(sys.argv[1],'rb').read()).decode(),
                  "ts":1787610000000}))
PY
mcp assets.put_asset "$TMP/asset.json"; echo

# ---------------------------------------------------------------------------------------------
say "4/8  the OUTBOX emails it — a durable, retried, capability-gated effect"
python3 - > "$TMP/outbox.json" <<'PY'
import json, time
payload = {"workspace":"acme","recipients":["alerts@nube-io.com"],
           "subject":"NEM12 interval data — ZZZZ035361",
           "body":"Automated meter export attached. 4 channels, 15-minute intervals.",
           "assetId":"nem12-demo-export",
           "filename":"ZZZZ035361_nem12#0045575584#TCAUSTM.csv","mime":"text/csv"}
# A fresh effect id per run, so the relay sends again rather than deduping.
print(json.dumps({"id":f"mail-demo-{int(time.time())}","target":"email","action":"meter-export",
                  "payload":json.dumps(payload),"ts":1787610000000}))
PY
mcp outbox.enqueue "$TMP/outbox.json"; echo
echo "   the relay ticks every 2s — watch the node log for 'email sent'"

# ---------------------------------------------------------------------------------------------
say "5/8  seal the mailbox credential (a PATH on the record; the value lives in secrets)"
cat > "$TMP/secret.json" <<EOF
{"path":"mail/demo-mailbox","value":"$MAILBOX","visibility":"workspace"}
EOF
mcp secret.set "$TMP/secret.json"; echo

# ---------------------------------------------------------------------------------------------
say "6/8  register the mail SOURCE"
cat > "$TMP/source.json" <<'EOF'
{
  "id": "mail-demo",
  "name": "Mail demo mailbox",
  "host": "127.0.0.1", "port": 3143, "tls": "none", "mailbox": "INBOX",
  "username": "alerts@nube-io.com", "auth": "plain",
  "secretPath": "mail/demo-mailbox",
  "channel": "mail", "pollSeconds": 15,
  "allowSenders": ["@example.com"],
  "attachments": {
    "storeBytes": true, "ingest": true, "format": "auto",
    "extensions": ["csv"], "seriesPrefix": "nem12.", "offsetMinutes": 600
  }
}
EOF
mcp mail.source.register "$TMP/source.json" >/dev/null
echo '{"id":"mail-demo"}' > "$TMP/id.json"
echo "   registered. proving the credentials without importing anything:"
mcp mail.source.check "$TMP/id.json" | python3 -m json.tool

# ---------------------------------------------------------------------------------------------
say "7/8  import it"
cat > "$TMP/show_pass.py" <<'PY'
import sys, json
p = json.load(sys.stdin)["pass"]
print("   fetched={fetched} imported={imported} duplicates={duplicates} "
      "rejected={rejected} samples={samples}".format(**p))
PY
for _ in $(seq 1 20); do
  PASS=$(mcp mail.source.poll "$TMP/id.json")
  echo "$PASS" | python3 "$TMP/show_pass.py"
  echo "$PASS" | grep -q '"imported": *[1-9]' && break
  sleep 3
done

# ---------------------------------------------------------------------------------------------
say "8/8  what landed"
echo '{"channel":"mail"}' > "$TMP/ch.json"

# The reporters live in files rather than `python3 -c '…'`: an f-string cannot carry a backslash,
# and shell quoting inside `-c` forces exactly that. One less thing to get subtly wrong.
cat > "$TMP/show_inbox.py" <<'PY'
import sys, json
items = json.load(sys.stdin)["items"]
print("   lb inbox — {} item(s) on channel `mail`:".format(len(items)))
for it in items[-3:]:
    m = it["meta"]
    print("     * {}".format(m["subject"]))
    print("       from {} - raw asset {}".format(m["from"]["address"], m["rawAssetId"]))
    for a in m["attachments"]:
        ing = a.get("ingest")
        extra = ""
        if ing:
            extra = " -> {}: {} samples into {} series".format(
                ing["format"], ing["accepted"], len(ing["series"]))
        print("       attachment {} ({} bytes){}".format(a["filename"], a["bytes"], extra))
PY
mcp inbox.list "$TMP/ch.json" | python3 "$TMP/show_inbox.py"

cat > "$TMP/show_stats.py" <<'PY'
import sys, json, datetime
s = json.load(sys.stdin)
tz = datetime.timezone(datetime.timedelta(hours=10))
fmt = lambda ms: datetime.datetime.fromtimestamp(ms / 1000, tz).strftime("%Y-%m-%d %H:%M AEST")
print("")
print("   series nem12.ZZZZ035361.B1 - {} samples, {} -> {}".format(
    s["raw_count"], fmt(s["first_ts"]), fmt(s["last_ts"])))
print("   producers: {}".format(s["producers"]))
print("   (re-run this script: a new message arrives, and raw_count does NOT grow -")
print("    a sample's dedup key comes from its timestamp, so overlapping files converge.)")
PY
echo '{"series":"nem12.ZZZZ035361.B1"}' > "$TMP/stats.json"
mcp series.stats "$TMP/stats.json" | python3 "$TMP/show_stats.py"

cat > "$TMP/build_read.py" <<'PY'
import json, datetime
tz = datetime.timezone(datetime.timedelta(hours=10))
f = int(datetime.datetime(2026, 7, 2, 0, 0, tzinfo=tz).timestamp() * 1000)
t = int(datetime.datetime(2026, 7, 3, 0, 0, tzinfo=tz).timestamp() * 1000)
print(json.dumps({"series": "nem12.ZZZZ035361.B1", "mode": "buckets",
                  "from": f, "to": t, "width_ms": 3600000, "method": "sum"}))
PY
python3 "$TMP/build_read.py" > "$TMP/read.json"

cat > "$TMP/show_day.py" <<'PY'
import sys, json, datetime
tz = datetime.timezone(datetime.timedelta(hours=10))
rows = json.load(sys.stdin)["buckets"]
print("")
print("   one local day, hourly (kWh) - a solar export curve, which is how you know the")
print("   NEM+10 offset and the period-ENDING convention are both right:")
for r in rows:
    v = r.get("value") or 0
    h = datetime.datetime.fromtimestamp(r["t"] / 1000, tz).strftime("%H:%M")
    print("     {}  {:<44} {}".format(h, "#" * int(v * 3), round(v, 3)))
PY
mcp series.read "$TMP/read.json" | python3 "$TMP/show_day.py"

say "done — stop the mail server with: docker rm -f lb-greenmail"
