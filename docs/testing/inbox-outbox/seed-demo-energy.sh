#!/usr/bin/env bash
# Seed an energy/insights-themed INBOX + OUTBOX demo into a RUNNING node — real records through the
# real host verbs (inbox.record / inbox.resolve / outbox.enqueue over POST /mcp/call), the exact
# paths production motion flows through (no fakes, rule 9). Gives a demo something to SHOW on the
# inbox channel view + the outbox/proof panel: energy alerts land as inbox items (some tagged
# needs:approval), a few are resolved (approved/rejected/deferred), and must-deliver effects
# (notify a facilities manager, open a work order, publish a report) sit in the outbox.
#
# It is the messaging sibling of the sqlite datasource seeder — same shape, same auth. The energy
# story matches the seeded `demo-buildings` dataset (site → meter → point → point_reading, kWh/kW/
# L·min/m³/temp) so the alerts read as if raised off that data:
#   docs/testing/datasources/README.md
#
# `ts` is a caller-injected LOGICAL timestamp (monotone per channel — no wall-clock in core, testing
# §3). We use small increasing integers so the channel view orders oldest→newest deterministically.
#
# Idempotent: every write is a stable-id UPSERT — re-running replaces the same rows, never dupes.
#
# Prereqs: a running `make dev` node + `jq` + `curl`. `user:ada`/`acme` is an admin (holds the
# author caps inbox.record / outbox.enqueue / inbox.resolve — see authz/builtin_roles.rs).
# Usage:  bash docs/testing/inbox-outbox/seed-demo-energy.sh [GATEWAY_URL] [USER] [WORKSPACE]
set -eu
GW="${1:-http://127.0.0.1:8080}"
USER="${2:-user:ada}"
WS="${3:-acme}"
CH="energy-ops"   # the channel the CHANNELS page (switcher rail) shows
APPROVALS="approvals"   # the channel the INBOX page (triage queue) shows — it defaults to `approvals`

command -v jq >/dev/null || { echo "seed-demo-energy needs jq"; exit 1; }

echo "-> login $GW as $USER/$WS"
TOKEN=$(curl -fsS -X POST "$GW/login" -H 'content-type: application/json' \
  -d "{\"user\":\"$USER\",\"workspace\":\"$WS\"}" | jq -r .token)
[ -n "$TOKEN" ] && [ "$TOKEN" != "null" ] || { echo "login failed (is $USER a member of $WS?)"; exit 1; }

call() { # tool, json-args  -> POST /mcp/call (fails loud on a denied/bad call)
  curl -fsS -X POST "$GW/mcp/call" -H "authorization: Bearer $TOKEN" \
    -H 'content-type: application/json' -d "{\"tool\":\"$1\",\"args\":$2}" >/dev/null
}
item() { # id, body, ts  — post a message onto the energy-ops channel.
  # NOTE: we use channel.post, NOT inbox.record. Both write the SAME inbox item (channel.post calls
  # lb_inbox::record under the hood), but channel.post ALSO registers the channel in the
  # channel_registry — which is what makes `energy-ops` appear in the UI channel switcher. A raw
  # inbox.record writes the item but leaves the channel unregistered → invisible in the list.
  call channel.post "{\"cid\":\"$CH\",\"id\":\"$1\",\"body\":$(jq -Rn --arg b "$2" '$b'),\"ts\":$3}"
}
approval() { # id, body, ts  — record an item on the `approvals` channel (the INBOX/triage queue).
  # The Inbox page reads inbox.list("approvals"); it does NOT need the channel registered, so a raw
  # inbox.record is enough here (unlike the Channels switcher, which needs channel.post).
  call inbox.record "{\"channel\":\"$APPROVALS\",\"id\":\"$1\",\"body\":$(jq -Rn --arg b "$2" '$b'),\"ts\":$3}"
}
resolve() { # item_id, decision(approved|rejected|deferred), ts
  call inbox.resolve "{\"item_id\":\"$1\",\"decision\":\"$2\",\"ts\":$3}"
}
effect() { # id, target, action, payload-json, ts  — enqueue a must-deliver outbox effect
  call outbox.enqueue "{\"id\":\"$1\",\"target\":\"$2\",\"action\":\"$3\",\"payload\":$4,\"ts\":$5}"
}

echo "-> inbox: energy alerts on channel '$CH'"
# Plain notices (informational — a channel view / unread count shows these).
item energy-1 "Site HQ-North total demand hit 412 kW at 14:15 — 18% above the last 30-day peak for this hour." 1
item energy-2 "Building B4 chiller CH-2 kW/ton drifted to 0.94 (baseline 0.71) over the last 6h — efficiency degrading." 2
item energy-3 "Water meter WM-07 (cooling tower makeup) flow 63 L/min sustained overnight — possible float-valve leak." 3
item energy-5 "Daily energy report ready: 8 sites, 4,910 kWh yesterday (+6.2% vs 7-day avg), peak 14:00–15:00." 6

# Items that gate a must-deliver action on a human sign-off (tagged in the body for the demo; the
# resolution facet below is the real approve/reject the workflow reads).
item energy-4 "[needs:approval] Demand-response event proposed for HQ-North 15:00–16:00: shed 60 kW by raising 3 AHU setpoints 1.5°C. Approve to dispatch." 4
item energy-6 "[needs:approval] After-hours HVAC runtime detected at Depot-West (Sat/Sun 22:00–05:00) — propose scheduling a maintenance work order." 7
item energy-7 "[needs:approval] Solar export at Roof-Array-2 fell to 0 kWh for 3 consecutive days — propose opening an inverter fault ticket." 8

echo "-> inbox: resolutions on the channel items (approve / reject / defer)"
resolve energy-4 approved 5   # DR event approved → the notify + report effects below are the follow-through
resolve energy-6 deferred 9   # after-hours: defer (review next week)
resolve energy-7 rejected 10  # solar: known planned inverter maintenance — rejected, no ticket

echo "-> approvals: pending items for the INBOX/triage queue (left UNRESOLVED so they show up)"
# These land on the `approvals` channel the Inbox page reads. Leave them pending — the Inbox page's
# whole job is the approve/reject queue, so a populated queue = unresolved items with the buttons live.
approval appr-dr    "Demand-response event: HQ-North 15:00–16:00 — shed 60 kW by raising 3 AHU setpoints 1.5°C. Approve to dispatch." 20
approval appr-work  "After-hours HVAC runtime at Depot-West (Sat/Sun 22:00–05:00) — schedule a maintenance work order?" 21
approval appr-solar "Solar export at Roof-Array-2 fell to 0 kWh for 3 consecutive days — open an inverter fault ticket?" 22
approval appr-leak  "Suspected leak: cooling-tower makeup WM-07 at 63 L/min overnight — dispatch a plumber?" 23

echo "-> outbox: must-deliver effects (the durable follow-through)"
# A notify effect (facilities manager) — the DR approval's consequence.
effect eff-notify-dr email notify \
  "$(jq -n -c '{to:"facilities@acme.example",subject:"DR event dispatched: HQ-North 15:00-16:00 (-60 kW)",body:"Approved by ada. 3 AHU setpoints +1.5C for one hour."}')" 11
# A work-order effect to a CMMS-style target.
effect eff-workorder-b4 workorder create \
  "$(jq -n -c '{asset:"B4/CH-2",priority:"high",title:"Chiller efficiency degraded (kW/ton 0.94)",due:"3d"}')" 12
# Publish the daily energy report downstream (a sync-style must-deliver).
effect eff-report-daily report publish \
  "$(jq -n -c '{report:"daily-energy",date:"yesterday",sites:8,total_kwh:4910,peak_window:"14:00-15:00"}')" 13
# A leak alert to the notify target (from the water-meter notice).
effect eff-notify-leak email notify \
  "$(jq -n -c '{to:"facilities@acme.example",subject:"Suspected leak: cooling-tower makeup WM-07",body:"63 L/min sustained overnight — check float valve."}')" 14

echo
echo "-> done. Seeded into '$WS':"
echo "   Channels page (channel '$CH'): 7 messages (3 tagged needs:approval), 3 resolved."
echo "   Inbox page (triage queue 'approvals'): 4 pending items with Approve/Reject."
echo "   outbox: 4 pending must-deliver effects (email notify ×2, workorder, report publish)."
echo
echo "   Read it back over the real read verbs:"
echo "     curl -fsS -X POST $GW/mcp/call -H \"authorization: Bearer \$TOKEN\" \\"
echo "       -H 'content-type: application/json' -d '{\"tool\":\"inbox.list\",\"args\":{\"channel\":\"$CH\"}}' | jq"
echo "     curl -fsS -X POST $GW/mcp/call -H \"authorization: Bearer \$TOKEN\" \\"
echo "       -H 'content-type: application/json' -d '{\"tool\":\"outbox.status\",\"args\":{}}' | jq"
