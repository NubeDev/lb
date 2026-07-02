# lazybones — the `node` binary (Rust/cargo) + the UI (React/Vite/pnpm) developer tasks.
#
# Lazybones is ONE binary built from shared crates; "edge" vs "cloud" is config and
# role, never a code branch (CLAUDE.md rule #1, README §3.1). So there is no
# edge-codebase and cloud-codebase — there are two ways to RUN the same `node`:
#   edge   a solo node: its own authority, fully offline, NO gateway. (the `edge` target)
#   cloud  the same node with the SSE/HTTP gateway mounted so a browser can reach it,
#          selected purely by setting LB_GATEWAY_ADDR. (the `cloud` target)
# That env-flag IS the role selection the thin wiring layer permits — see
# rust/node/src/main.rs.
#
# There are NO dependency containers to bring up: SurrealDB is embedded (SurrealKV for
# the dev/node targets below, kv-mem for tests unless they opt into a path) and
# Zenoh is embedded. So unlike the sibling rubix-cube Makefile there is deliberately
# NO `deps-up`/`docker` stack — a bare `make dev` runs against nothing external.
#
# New here? The browser-against-the-cloud-node path is the full live-node demo. The
# TL;DR:
#   make setup       one-time: pnpm install + add the wasm target
#   make dev         the cloud node (gateway) + the UI (browser build) together
#   make kill        free the dev ports if a previous run was left running
#
#   make setup       pnpm install in ui/ + ensure the wasm32-wasip2 target is present
#   make build       build the wasm guest(s) + the host workspace + the UI
#   make build-be    build the host workspace (after building the wasm guests)
#   make build-wasm  build just the S1 `hello` + S2 `hello-v2` wasm components
#   make build-ui    type-check + production-build the UI
#
#   make dev         cloud node + UI together (Ctrl-C stops both) — the demo loop
#   make edge        run JUST a solo node (no gateway, offline) — the edge posture
#   make cloud       run JUST the node with the SSE gateway (LB_GATEWAY_ADDR) — cloud
#   make ui          run JUST the UI dev server (browser build, points at the gateway)
#
#   make pack        build+sign an extension into .lazybones/extensions (EXT=hello-v2 by default)
#   make publish-ext pack + upload it to a RUNNING node (make cloud first) → installed + loaded live
#   make trusted-pubkey  print the dev publisher's LB_TRUSTED_PUBKEYS value (key auto-generated)
#
#   make test        cargo test (host) + vitest (UI)
#   make test-be     cargo test --workspace
#   make test-ui     pnpm test (vitest)
#   make lint        cargo clippy + UI type-check
#   make fmt         cargo fmt + (the UI has no formatter wired yet)
#   make fmt-check   cargo fmt --check (what CI runs)
#   make size        the FILE-LAYOUT ≤400-line check (the S0 CI gate)
#   make clean       remove build artifacts (cargo target + ui/dist)
#   make kill        free the dev ports + reap any orphaned node/cargo/vite
#
# See CLAUDE.md for the architecture and docs/STAGES.md for the staged build plan.

BE_DIR  := rust
UI_DIR  := ui
NODE_BIN := node

# Optional cargo features for the `node` binary in `make dev`/`make edge`. OFF by default (the
# external agent is opt-in per its scope). Turn Open Interpreter on for the live UI with either:
#   make dev EXTAGENT=1                       (shorthand → external-agent)
#   make dev NODE_FEATURES=external-agent     (explicit; add more comma-separated features here)
# When set, `make dev` ALSO checks `interpreter` is on PATH and warns if ZAI_API_KEY is unset (the
# Open Interpreter run needs both — see docs/skills/external-agent/SKILL.md's "three gates").
ifeq ($(EXTAGENT),1)
NODE_FEATURES ?= external-agent
endif
NODE_FEATURES ?=
NODE_FEATURE_FLAG := $(if $(NODE_FEATURES),--features $(NODE_FEATURES),)

# The wasm guest components the node loads. `hello` is the S1 spine extension; the
# node won't boot without it built (it reads the .wasm at startup). `hello-v2` is the
# S2 hot-reload swap target. Both build to wasm32-wasip2 --release.
WASM_EXTS := hello hello-v2
WASM_TARGET := wasm32-wasip2

# Dev ports — kept in sync with the code. The node mounts the SSE/HTTP gateway on
# GW_ADDR when LB_GATEWAY_ADDR is set (rust/node/src/main.rs); the UI's browser build
# points VITE_GATEWAY_URL at it. The Vite dev server listens on UI_PORT (strictPort
# 5173 in vite.config.ts — change it there too if you override this). We only track
# the ports `make kill` must free.
GW_HOST ?= 127.0.0.1
GW_PORT ?= 8080
GW_ADDR := $(GW_HOST):$(GW_PORT)
GW_URL  := http://$(GW_HOST):$(GW_PORT)
UI_PORT ?= 5173

# The workspace the node serves. One workspace is enough for the demo (= tenant).
WS ?= acme

# The dev identity the node seeds as a `workspace-admin` member of $(WS) at boot (global-identity
# scope). The login gate requires membership, so the node boot-straps this identity into the workspace
# (provisioning + joining — NOT a login bypass). Override with the handle you log in as; clear it
# (LB_SEED_USER=) to skip seeding entirely.
SEED_USER ?= user:ada

# Datasources (federation native extension). Setting FED_ENDPOINTS installs + supervises the
# `federation` sidecar at boot and pre-approves these `host:port` endpoints (`net:tls:host:port`).
# The seed pre-registers one source so the Datasources page works on first boot. Defaults target the
# dev TimescaleDB from docker/postgres/docker-compose.yml (port 5433). Override or clear to disable:
#   make dev FED_ENDPOINTS=         (no federation sidecar)
# Requires the postgres-featured sidecar binary — built by the `federation` target below.
FED_ENDPOINTS  ?= 127.0.0.1:5433
FED_SEED_NAME  ?= timescale
FED_SEED_KIND  ?= postgres
FED_SEED_EP    ?= 127.0.0.1:5433
FED_SEED_DSN   ?= host=127.0.0.1 port=5433 user=lb password=lb_secret dbname=lb sslmode=disable
# The env block passed to the node for the federation role (empty when FED_ENDPOINTS is cleared).
FED_ENV = $(if $(FED_ENDPOINTS),LB_FEDERATION_ENDPOINTS="$(FED_ENDPOINTS)" LB_FEDERATION_SEED_NAME="$(FED_SEED_NAME)" LB_FEDERATION_SEED_KIND="$(FED_SEED_KIND)" LB_FEDERATION_SEED_ENDPOINT="$(FED_SEED_EP)" LB_FEDERATION_SEED_DSN="$(FED_SEED_DSN)",)

# All persistent local dev state lives under ONE root: `.lazybones/` (renamed from the too-generic
# `.data/`). The node store, the dev publisher key, and packaged artifacts are subdirs of it, so a
# single `rm -rf .lazybones` resets a dev box and one `.gitignore` line covers everything.
#   .lazybones/data/dev-store      the SurrealKV node store (LB_STORE_PATH)
#   .lazybones/keys/dev-publisher  the dev publisher Ed25519 seed (lb-pack reads/creates it)
#   .lazybones/extensions          packaged signed artifacts (lb-pack --out)
# Override the root with: make dev LB_DIR=/path/to/state
LB_DIR     ?= $(CURDIR)/.lazybones
STORE_DIR  ?= $(LB_DIR)/data
STORE_PATH ?= $(STORE_DIR)/dev-store
KEY_DIR    ?= $(LB_DIR)/keys
KEY_FILE   ?= $(KEY_DIR)/dev-publisher.key
ART_DIR    ?= $(LB_DIR)/extensions

# The dev publisher key id paired with KEY_FILE (must match what lb-pack stamps into the artifact).
PUBLISHER_ID ?= dev-publisher
# The extension the dev pack/publish targets operate on (the hot-reload swap demo). Its manifest +
# built wasm.
EXT          ?= hello-v2
EXT_MANIFEST := $(BE_DIR)/extensions/$(EXT)/extension.toml
EXT_WASM     := $(BE_DIR)/extensions/$(EXT)/target/$(WASM_TARGET)/release/$(subst -,_,$(EXT))_ext.wasm
EXT_ARTIFACT := $(ART_DIR)/$(EXT).artifact.json
# The extension's built federated UI bundle (the half the signed artifact does NOT carry) and the dir
# the running node serves it from (LB_EXT_UI_DIR default = `extensions-ui/` relative to BE_DIR, i.e.
# where the node is launched). `publish-ext` copies the dist here so the page's `remoteEntry.js` is
# reachable — publishing the wasm tool alone leaves the UI 404ing.
EXT_UI_DIST  := $(BE_DIR)/extensions/$(EXT)/ui/dist
EXT_UI_SERVE := $(BE_DIR)/extensions-ui/$(EXT)

.PHONY: setup build build-be build-wasm build-ui \
        dev edge cloud ui ui-preview pack publish-ext trusted-pubkey seed-thecrew \
        test test-be test-ui lint fmt fmt-check size clean kill

# One-time setup: install the UI deps and make sure the wasm target is installed (the
# rust-toolchain.toml already pins it, but `rustup target add` is idempotent and saves
# a confusing first-build failure on a fresh box).
setup:
	rustup target add $(WASM_TARGET) || true
	cd $(UI_DIR) && pnpm install
	@echo "setup done — now: make dev"

build: build-wasm build-be build-ui

# Build the wasm guest components first — the host workspace builds without them, but
# the `node` binary reads the built hello.wasm at startup, so a run needs these.
build-wasm:
	@for ext in $(WASM_EXTS); do \
		echo "→ building wasm guest: $$ext"; \
		( cd $(BE_DIR)/extensions/$$ext && cargo build --target $(WASM_TARGET) --release ) || exit $$?; \
	done

build-be: build-wasm
	cd $(BE_DIR) && cargo build --workspace

build-ui:
	cd $(UI_DIR) && pnpm install && pnpm build

# The demo loop: the cloud node (gateway mounted) + the UI browser build pointed at
# it, in ONE foreground process group so Ctrl-C (or `make kill`) stops both. The trap
# reaps the children on exit so no orphan keeps a port held. Builds the wasm guest
# first (the node needs it at startup).
dev: build-wasm trusted-pubkey federation
	@mkdir -p $(STORE_DIR)
	@echo "node gateway → $(GW_URL)   UI → http://127.0.0.1:$(UI_PORT)   (ws=$(WS), store=$(STORE_PATH))"
	@echo "datasources → federation sidecar endpoints: $(if $(FED_ENDPOINTS),$(FED_ENDPOINTS),<disabled>)"
	@echo "node features → $(if $(NODE_FEATURES),$(NODE_FEATURES),<none> (external agent OFF; set EXTAGENT=1 to enable))"
	@if [ -n "$(NODE_FEATURES)" ]; then \
	  command -v interpreter >/dev/null 2>&1 || echo "⚠ external-agent on but 'interpreter' is not on PATH — Open Interpreter runs will fail"; \
	  [ -n "$$ZAI_API_KEY" ] || echo "⚠ external-agent on but ZAI_API_KEY is unset — the model call will fail"; \
	fi
	@trap 'kill 0' EXIT INT TERM; \
	TRUSTED=$$($(BE_DIR)/target/debug/lb-pack pubkey $(KEY_FILE) --key-id $(PUBLISHER_ID)); \
	( cd $(BE_DIR) && LB_GATEWAY_ADDR=$(GW_ADDR) LB_WORKSPACE=$(WS) LB_STORE_PATH=$(STORE_PATH) LB_SEED_USER=$(SEED_USER) LB_TRUSTED_PUBKEYS=$$TRUSTED $(FED_ENV) cargo run -p $(NODE_BIN) $(NODE_FEATURE_FLAG) ) & \
	( cd $(UI_DIR) && VITE_GATEWAY_URL=$(GW_URL) pnpm run dev ) & \
	wait

# Build the federation sidecar with the Postgres connector (the headline datasource). The node's
# federation role spawns this binary at boot; it links a heavy DB engine + vendored OpenSSL, so it is
# its own target (a no-op when current). `make dev` depends on it; build alone with `make federation`.
.PHONY: federation
federation:
	cd $(BE_DIR) && cargo build -p federation --features postgres

# ---------------------------------------------------------------------------------------------------
# Reproducible Linux (cross) builds in Docker — see docker/build/. Use these when the host C toolchain
# is broken (e.g. a half-installed zig hijacks the linker: `zigranlib: not found`). The image carries
# real GCC cross-toolchains so vendored OpenSSL (the federation crate) compiles per target.
#   make docker-build-image                     build the toolchain image once
#   make docker-build                           build the node for linux x86_64 (default)
#   make docker-build TARGET=linux-arm64        aarch64 (64-bit Pi)
#   make docker-build TARGET=linux-armv7        armv7 (32-bit Pi)
#   make docker-build TARGET=windows-x86_64     node.exe
#   make docker-build TARGET=deb                .deb package
#   make docker-build PKG=federation FEATURES=postgres   the federation sidecar
DOCKER_BUILD_IMAGE ?= lazybones-build
TARGET ?= linux-x86_64
# A named volume keeps the cargo registry/git cache warm across runs (incremental builds).
DOCKER_CARGO_VOL   ?= lazybones-cargo-cache

.PHONY: docker-build-image docker-build
docker-build-image:
	docker build -t $(DOCKER_BUILD_IMAGE) docker/build

# Mounts the rust workspace at /work and a named cargo-cache volume; threads PKG/PROFILE/FEATURES
# through to build.sh. Output lands in $(BE_DIR)/target/<triple>/<profile>/ on the host.
docker-build:
	docker run --rm \
		-v $(CURDIR)/$(BE_DIR):/work \
		-v $(DOCKER_CARGO_VOL):/usr/local/cargo/registry \
		-e PKG="$(PKG)" -e PROFILE="$(PROFILE)" -e FEATURES="$(FEATURES)" \
		$(DOCKER_BUILD_IMAGE) $(TARGET)

# EDGE posture: a solo node — its own authority, fully offline, NO gateway. This is
# the same binary as `cloud`, just without LB_GATEWAY_ADDR set, so it runs the solo
# spine demo and exits/serves locally only. No browser reaches it (that's the point).
edge: build-wasm
	@mkdir -p $(STORE_DIR)
	@echo "edge: solo node (no gateway, offline)   (ws=$(WS), store=$(STORE_PATH))"
	cd $(BE_DIR) && LB_WORKSPACE=$(WS) LB_STORE_PATH=$(STORE_PATH) LB_SEED_USER=$(SEED_USER) cargo run -p $(NODE_BIN)

# CLOUD posture: the SAME binary with the SSE/HTTP gateway mounted (LB_GATEWAY_ADDR).
# A browser can now reach it. Run `make ui` (or `make dev`) against this.
cloud: build-wasm trusted-pubkey federation
	@mkdir -p $(STORE_DIR)
	@echo "cloud: node + gateway → $(GW_URL)   (ws=$(WS), store=$(STORE_PATH))"
	@echo "datasources → federation sidecar endpoints: $(if $(FED_ENDPOINTS),$(FED_ENDPOINTS),<disabled>)"
	TRUSTED=$$($(BE_DIR)/target/debug/lb-pack pubkey $(KEY_FILE) --key-id $(PUBLISHER_ID)); \
	cd $(BE_DIR) && LB_GATEWAY_ADDR=$(GW_ADDR) LB_WORKSPACE=$(WS) LB_STORE_PATH=$(STORE_PATH) LB_SEED_USER=$(SEED_USER) LB_TRUSTED_PUBKEYS=$$TRUSTED $(FED_ENV) cargo run -p $(NODE_BIN)

# Just the UI dev server, browser build, pointed at the gateway. Pair with `make
# cloud` in another terminal.
ui:
	cd $(UI_DIR) && VITE_GATEWAY_URL=$(GW_URL) pnpm run dev

# ---------------------------------------------------------------------------------------------------
# Extension dev flow: build → pack (sign) → publish (upload, which installs + loads on the server).
# `lb-pack` is the bridge build.sh never had: it turns a built *.wasm + extension.toml into the SIGNED
# Artifact JSON the gateway's `POST /extensions` and the UI's UploadArtifact accept. The dev publisher
# key lives at $(KEY_FILE) (generated on first use); its public half is trusted by the node via
# LB_TRUSTED_PUBKEYS (the `dev`/`cloud` targets wire it from `lb-pack pubkey`). Trust is the
# environment, never the upload — that split is the whole point.

# Build the lb-pack tool (the dev packager). Cheap once built; the run targets depend on it.
$(BE_DIR)/target/debug/lb-pack:
	cd $(BE_DIR) && cargo build -p lb-pack

# Print the dev publisher's `key_id=hexpubkey` (generating the key on first run). This IS the value
# the node wants in LB_TRUSTED_PUBKEYS; the `dev`/`cloud` targets capture it automatically.
trusted-pubkey: $(BE_DIR)/target/debug/lb-pack
	@$(BE_DIR)/target/debug/lb-pack pubkey $(KEY_FILE) --key-id $(PUBLISHER_ID)

# Build $(EXT)'s wasm and package it into a signed artifact at $(EXT_ARTIFACT). Pure local: produces
# the file the UI can upload OR `make publish-ext` can POST. Override the target ext with EXT=<name>.
pack: $(BE_DIR)/target/debug/lb-pack
	@echo "→ building wasm guest: $(EXT)"
	@( cd $(BE_DIR)/extensions/$(EXT) && cargo build --target $(WASM_TARGET) --release )
	@mkdir -p $(ART_DIR)
	$(BE_DIR)/target/debug/lb-pack $(EXT_MANIFEST) $(EXT_WASM) $(KEY_FILE) \
		--key-id $(PUBLISHER_ID) --out $(EXT_ARTIFACT)

# Publish $(EXT) to a RUNNING node ($(GW_URL)): pack it, log in for a session token (the dev-login
# grants ext.publish), then POST the artifact. `204` ⇒ verified, installed, and LOADED live — the
# extension is reachable immediately (no restart). Needs `make cloud`/`make dev` running first, plus
# curl + jq. The node must trust this publisher key (the run targets set LB_TRUSTED_PUBKEYS for you).
publish-ext: pack
	@command -v jq >/dev/null || { echo "publish-ext needs jq"; exit 1; }
	@echo "→ login $(GW_URL) as dev/$(WS)"
	@TOKEN=$$(curl -fsS -X POST $(GW_URL)/login -H 'content-type: application/json' \
		-d '{"user":"dev","workspace":"$(WS)"}' | jq -r .token); \
	echo "→ POST $(GW_URL)/extensions ($(EXT))"; \
	code=$$(curl -sS -o /tmp/lb-publish-resp -w '%{http_code}' -X POST $(GW_URL)/extensions \
		-H "authorization: Bearer $$TOKEN" -H 'content-type: application/json' \
		--data-binary @$(EXT_ARTIFACT)); \
	echo "← HTTP $$code"; \
	if [ "$$code" = "204" ]; then echo "published + installed + loaded: $(EXT)"; \
	else echo "FAILED ($$code): $$(cat /tmp/lb-publish-resp)"; exit 1; fi
	@# Deploy the federated UI bundle. The signed artifact carries ONLY the wasm + manifest; the node
	@# serves the page from LB_EXT_UI_DIR ($(EXT_UI_SERVE)). Without this the sidebar entry appears but
	@# `remoteEntry.js` 404s. Skipped (with a note) when the extension ships no `ui/dist` (backend-only).
	@if [ -d "$(EXT_UI_DIST)" ]; then \
		echo "-> deploy UI bundle -> $(EXT_UI_SERVE)"; \
		rm -rf "$(EXT_UI_SERVE)"; mkdir -p "$(EXT_UI_SERVE)"; \
		cp -r "$(EXT_UI_DIST)"/* "$(EXT_UI_SERVE)"/; \
		echo "  UI deployed ($(GW_URL)/extensions/$(EXT)/ui/assets/remoteEntry.js)"; \
		echo "  NOTE: extension pages load only in the BUILT shell -- use 'make ui-preview', not 'make ui'."; \
	else echo "-> no ui/dist for $(EXT) -- skipping UI deploy"; fi

# Seed the thecrew (Graphics) demo into a RUNNING node: the AHU-1 scene doc + its bound `ahu1.*`
# series + a read-only "Graphics Scene" dashboard, all through the REAL host verbs (assets.put_doc /
# ingest / dashboard.save). Idempotent. Run `make publish-ext EXT=thecrew` first so the extension is
# installed (its grant carries the assets.* caps); the seed logs in as $(SEED_USER) (a member of $(WS))
# — NOT `dev` — because a live scene save/load needs the member `assets.*` grant (session findings).
seed-thecrew:
	bash $(BE_DIR)/extensions/thecrew/seed-demo.sh $(GW_URL) $(SEED_USER) $(WS)

# Serve the BUILT shell so extension pages actually load. The `dev`/`ui` targets run the Vite DEV
# server, where @originjs/vite-plugin-federation's host runtime is absent -- every federated remote
# fails with `getUrl(...).then is not a function`. A federated remote needs a production build.
# See debugging/extensions/federated-remote-fails-in-dev-server.md.
ui-preview:
	cd $(UI_DIR) && pnpm install && VITE_GATEWAY_URL=$(GW_URL) pnpm exec vite build
	cd $(UI_DIR) && VITE_GATEWAY_URL=$(GW_URL) pnpm exec vite preview --host 127.0.0.1 --port 4173

test: test-be test-ui

test-be:
	cd $(BE_DIR) && cargo test --workspace

test-ui:
	cd $(UI_DIR) && pnpm test

lint:
	cd $(BE_DIR) && cargo clippy --all-targets -- -D warnings
	cd $(UI_DIR) && pnpm exec tsc --noEmit

fmt:
	cd $(BE_DIR) && cargo fmt

# What CI enforces — fmt must be clean.
fmt-check:
	cd $(BE_DIR) && cargo fmt --all --check

# The FILE-LAYOUT ≤400-line gate (the S0 CI check). One responsibility per file.
size:
	bash $(BE_DIR)/scripts/check-file-size.sh

# Remove build artifacts — the cargo target and the UI build output. Leaves
# node_modules alone (re-run `make setup` / `pnpm install` to refresh those).
clean:
	cd $(BE_DIR) && cargo clean
	rm -rf $(UI_DIR)/dist
	@echo "cleaned cargo target + ui/dist (node_modules kept)"

# Free the dev ports AND reap any orphaned node/cargo/vite left by a crashed run.
# A crashed `make dev` never fires its trap: the children reparent to init and an
# orphaned `cargo run`/`node`/`vite` keeps holding $(GW_PORT)/$(UI_PORT), so the next
# boot fails on the port — not just a stale process. fuser is absent on some boxes, so
# we ALSO drive the kill off pkill by process signature.
#
# The pkill patterns lead with a bracket class on the first char (`[c]argo`, `[v]ite`)
# so the pattern STRING has no literal match for itself — without it pkill SIGKILLs its
# OWN shell (make expands the recipe into that shell's argv, which then contains the
# pattern text). `[c]argo` is still a regex that matches the real `cargo` process. The
# `-`/`||true` prefixes keep make from failing when a tool is missing or nothing
# matches.
#
# SIGTERM first, then poll pgrep until the processes exit, escalating stragglers to
# SIGKILL after ~5s. The wait doubles as the graceful-shutdown grace period AND closes
# the race where a returning `make kill` immediately followed by `make dev` raced the
# dying process for the port. We signal the cargo wrapper AND the compiled `node`
# binary separately: SIGTERM-ing `cargo run` does NOT forward to the child binary
# (cargo doesn't propagate it), so an orphaned node would otherwise keep the port.
kill:
	-@fuser -TERM -k $(GW_PORT)/tcp 2>/dev/null || true
	-@fuser -TERM -k $(UI_PORT)/tcp 2>/dev/null || true
	-@pkill -TERM -f '[c]argo run' 2>/dev/null || true
	-@pkill -TERM -f 'target/[d]ebug/node' 2>/dev/null || true
	-@pkill -TERM -f '[v]ite' 2>/dev/null || true
	@i=0; \
	while pgrep -f 'target/[d]ebug/node' >/dev/null 2>&1 \
	   || pgrep -f '[c]argo run' >/dev/null 2>&1 \
	   || pgrep -f '[v]ite' >/dev/null 2>&1; do \
		i=$$((i+1)); \
		if [ $$i -ge 50 ]; then \
			pkill -KILL -f '[c]argo run' 2>/dev/null || true; \
			pkill -KILL -f 'target/[d]ebug/node' 2>/dev/null || true; \
			pkill -KILL -f '[v]ite' 2>/dev/null || true; \
			break; \
		fi; \
		sleep 0.1; \
	done
	@echo "freed ports $(GW_PORT)/$(UI_PORT) and killed any orphaned node/cargo/vite"
