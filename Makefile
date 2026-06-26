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
# There are NO dependency containers to bring up: SurrealDB is embedded (kv-mem) and
# Zenoh is embedded. So unlike the sibling rubix-cube Makefile there is deliberately
# NO `deps-up`/`docker` stack — a bare `make dev` runs against nothing external.
#
# New here? The browser-against-the-cloud-node path is the only FULL end-to-end demo
# today (only the channel/messaging slice is wired through to a live node; the other
# views run on in-memory fakes until their gateway routes land). The TL;DR:
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

.PHONY: setup build build-be build-wasm build-ui \
        dev edge cloud ui \
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
dev: build-wasm
	@echo "node gateway → $(GW_URL)   UI → http://127.0.0.1:$(UI_PORT)   (ws=$(WS))"
	@trap 'kill 0' EXIT INT TERM; \
	( cd $(BE_DIR) && LB_GATEWAY_ADDR=$(GW_ADDR) LB_WORKSPACE=$(WS) cargo run -p $(NODE_BIN) ) & \
	( cd $(UI_DIR) && VITE_GATEWAY_URL=$(GW_URL) pnpm run dev ) & \
	wait

# EDGE posture: a solo node — its own authority, fully offline, NO gateway. This is
# the same binary as `cloud`, just without LB_GATEWAY_ADDR set, so it runs the solo
# spine demo and exits/serves locally only. No browser reaches it (that's the point).
edge: build-wasm
	@echo "edge: solo node (no gateway, offline)   (ws=$(WS))"
	cd $(BE_DIR) && LB_WORKSPACE=$(WS) cargo run -p $(NODE_BIN)

# CLOUD posture: the SAME binary with the SSE/HTTP gateway mounted (LB_GATEWAY_ADDR).
# A browser can now reach it. Run `make ui` (or `make dev`) against this.
cloud: build-wasm
	@echo "cloud: node + gateway → $(GW_URL)   (ws=$(WS))"
	cd $(BE_DIR) && LB_GATEWAY_ADDR=$(GW_ADDR) LB_WORKSPACE=$(WS) cargo run -p $(NODE_BIN)

# Just the UI dev server, browser build, pointed at the gateway. Pair with `make
# cloud` in another terminal. Without VITE_GATEWAY_URL the UI falls back to its
# in-memory fakes (the invoke seam in ui/src/lib/ipc/invoke.ts chooses the transport).
ui:
	cd $(UI_DIR) && VITE_GATEWAY_URL=$(GW_URL) pnpm run dev

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
