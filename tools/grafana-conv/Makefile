# grafana-conv/Makefile — developer tasks for the standalone Grafana → our-dashboard
# converter (scope: docs/scope/frontend/dashboard/grafana-conversion-scope.md, Stage 0).
#
# This is a DELIBERATELY SEPARATE Cargo workspace + separate UI from `rust/` and
# `ui/`. The standalone shape is what proves the mapping against real exports with
# zero core surface — no token, no workspace, no cap, no store, no bus. So unlike
# the sibling top-level Makefile there is intentionally NO gateway/store/extension
# machinery here, and NO dependency containers to bring up. Folding into the host
# behind a real `dashboard.import` verb is Stage 2 (viz/import-export-scope.md).
#
# New here? The TL;DR:
#   make setup       one-time: pnpm install --ignore-workspace in ui/
#   make dev         the Axum seam + the UI together (Ctrl-C stops both) — the demo loop
#   make kill        free the dev ports if a previous run was left running
#
#   make setup       pnpm install in ui/ (standalone — outside the repo pnpm workspace)
#   make build       cargo build the workspace + production-build the UI
#   make build-be    cargo build --workspace (the mapper crate + the app seam)
#   make build-ui    tsc --noEmit + vite build the UI
#
#   make dev         the seam (POST /convert) + the UI together — the demo loop
#   make seam        run JUST the Axum seam (the app binary)
#   make ui          run JUST the UI dev server (proxies /convert to the seam)
#
#   make test        cargo test --workspace + vitest
#   make test-be     cargo test --workspace (mapper unit + golden + mirror-sync + app route)
#   make test-ui     vitest run (component test renders REAL mapper output)
#   make lint        cargo clippy + UI type-check
#   make fmt         cargo fmt (the vendored model.rs mirror is skipped — see fmt-check)
#   make fmt-check   cargo fmt --all --check (what CI runs)
#   make fixture     regenerate the UI test fixture from the REAL mapper (not hand-written)
#   make clean       cargo clean + ui/dist + ui/node_modules
#   make kill        free the dev ports + reap any orphaned cargo/vite
#   make help        this list, annotated
#
# See README.md for the architecture. The vendored model.rs mirror is byte-synced
# to rust/crates/host/src/dashboard/model.rs and guarded by tests/mirror_sync.rs.

# Resolve paths regardless of where make is invoked from (mirrors desktop/Makefile).
THIS_DIR  := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
REPO_ROOT := $(abspath $(THIS_DIR)/..)
BE_DIR    := $(THIS_DIR)
UI_DIR    := $(THIS_DIR)/ui

# Dev ports — kept in sync with the code. The app binary serves POST /convert on
# SEAM_ADDR (app/src/main.rs: `GRAFANA_CONV_ADDR`, default 127.0.0.1:7878). The
# Vite dev server listens on UI_PORT and proxies /convert to the seam
# (ui/vite.config.ts). We only track the ports `make kill` must free.
SEAM_HOST ?= 127.0.0.1
SEAM_PORT ?= 7878
SEAM_ADDR := $(SEAM_HOST):$(SEAM_PORT)
SEAM_URL  := http://$(SEAM_HOST):$(SEAM_PORT)
UI_PORT   ?= 5173

# pnpm install flag: this dir is NOT a member of the repo-root pnpm workspace
# (pnpm-workspace.yaml lists ui/, packages/*, app/sdk only), so install standalone
# to avoid pnpm walking up and treating the root as the workspace root.
PNPM_INSTALL := pnpm install --ignore-workspace

.PHONY: setup build build-be build-ui dev seam ui \
        test test-be test-ui lint fmt fmt-check fixture clean kill help
.DEFAULT_GOAL := help

help: ## Show this help.
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*## "}{printf "  %-14s %s\n",$$1,$$2}'

# One-time setup: install the UI deps (standalone). No rustup step — the host
# toolchain already has the host triple; this workspace targets nothing else.
setup: ## One-time: pnpm install (standalone) in ui/.
	cd $(UI_DIR) && $(PNPM_INSTALL)
	@echo "setup done — now: make dev"

build: build-be build-ui ## cargo build the workspace + production-build the UI.

build-be: ## cargo build the workspace (the mapper crate + the Axum app seam).
	cd $(BE_DIR) && cargo build --workspace

build-ui: ## tsc --noEmit + vite production-build the UI.
	cd $(UI_DIR) && pnpm exec tsc --noEmit && pnpm exec vite build

# The demo loop: the Axum seam + the UI browser build pointed at it, in ONE
# foreground process group so Ctrl-C (or `make kill`) stops both. The trap reaps
# the children on exit so no orphan keeps a port held (same idiom as the repo-root
# Makefile's `dev` target).
dev: ## The demo loop: the Axum seam + the UI together (Ctrl-C stops both).
	@echo "seam → $(SEAM_URL)   UI → http://127.0.0.1:$(UI_PORT)   (UI proxies /convert)"
	@echo "open the printed UI URL, paste/drop a Grafana .json, hit Convert"
	@trap 'kill 0' EXIT INT TERM; \
	( cd $(BE_DIR) && GRAFANA_CONV_ADDR=$(SEAM_ADDR) cargo run -p grafana-conv-app ) & \
	( cd $(UI_DIR) && pnpm run dev ) & \
	wait

# JUST the Axum seam. Pair with `make ui` in another terminal.
seam: ## Run JUST the Axum seam (POST /convert). Pair with `make ui`.
	@echo "seam → $(SEAM_URL)   (POST /convert)"
	cd $(BE_DIR) && GRAFANA_CONV_ADDR=$(SEAM_ADDR) cargo run -p grafana-conv-app

# JUST the UI dev server (proxies /convert to the seam at $(SEAM_URL)). Pair with
# `make seam` in another terminal.
ui: ## Run JUST the UI dev server (proxies /convert to the seam).
	cd $(UI_DIR) && pnpm run dev

test: test-be test-ui ## cargo test --workspace + vitest.

# Backend AUTOMATED tests. Green on clean master: mapper unit (11) + golden
# fixtures (5: fleet-overview, rows-collapsed, rows-expanded, advanced-vars,
# report-completeness) + mirror-sync guard (1) + app route (2) + doctest (1).
# The mirror-sync guard reads the host's model.rs at ../../../rust/crates/host/...,
# so this target assumes the repo layout is intact (it is — this is a subdir).
test-be: ## cargo test --workspace (mapper + golden + mirror-sync + app route + doctest).
	cd $(BE_DIR) && cargo test --workspace

# UI component test. Renders REAL mapper output captured as a fixture (regenerate
# with `make fixture`). NOT a fake backend: the data is real mapper output, captured.
test-ui: ## vitest run (component test renders REAL mapper output).
	cd $(UI_DIR) && pnpm exec vitest run

lint: ## cargo clippy (deny warnings) + UI tsc --noEmit.
	cd $(BE_DIR) && cargo clippy --workspace --all-targets -- -D warnings
	cd $(UI_DIR) && pnpm exec tsc --noEmit

# cargo fmt the workspace. The vendored mapper/src/model.rs is byte-synced to the
# host's unformatted copy and is NOT reformatted here — `#[rustfmt::skip]` on its
# `mod model;` declaration keeps fmt off it, and tests/mirror_sync.rs fails loudly
# if anyone reformats it anyway.
fmt: ## cargo fmt --all (the vendored model.rs mirror is skipped).
	cd $(BE_DIR) && cargo fmt --all

# What CI enforces — fmt must be clean.
fmt-check: ## cargo fmt --all --check (what CI runs).
	cd $(BE_DIR) && cargo fmt --all --check

# Regenerate the UI test fixture from the REAL mapper. Run after changing the
# mapper's output shape (the component test asserts on the captured JSON, so a
# shape change needs a re-capture — NOT a hand-edit of the fixture).
fixture: ## Regenerate the UI test fixture from the REAL mapper (not hand-written).
	cd $(BE_DIR) && cargo run -p grafana-conv-mapper --example emit_fixture > $(UI_DIR)/src/test/sample-output.json
	@echo "regenerated $(UI_DIR)/src/test/sample-output.json from real mapper output"

# Remove build artifacts — cargo target, the UI build output, and the UI deps.
# Leaves the vendored model.rs and fixtures alone (those are source, not artifacts).
clean: ## Remove cargo target + ui/dist + ui/node_modules.
	cd $(BE_DIR) && cargo clean
	rm -rf $(UI_DIR)/dist $(UI_DIR)/node_modules
	@echo "cleaned cargo target + ui/dist + ui/node_modules"

# Free the dev ports AND reap any orphaned cargo/vite left by a crashed `make dev`.
# Same idiom as the repo-root Makefile's `kill`: bracket-class the pkill patterns
# (`[c]argo`, `[v]ite`) so the pattern STRING has no literal match for itself —
# without it pkill SIGKILLs its OWN shell. SIGTERM first, escalate to SIGKILL
# after ~5s. fuser is absent on some boxes, so pkill by signature is the backbone.
kill: ## Free the dev ports + reap any orphaned cargo/vite.
	-@fuser -TERM -k $(SEAM_PORT)/tcp 2>/dev/null || true
	-@fuser -TERM -k $(UI_PORT)/tcp 2>/dev/null || true
	-@pkill -TERM -f 'grafana-conv-app' 2>/dev/null || true
	-@pkill -TERM -f 'target/[d]ebug/grafana-conv' 2>/dev/null || true
	-@pkill -TERM -f '[v]ite' 2>/dev/null || true
	@i=0; \
	while pgrep -f 'target/[d]ebug/grafana-conv' >/dev/null 2>&1 \
	   || pgrep -f '[v]ite' >/dev/null 2>&1; do \
		i=$$((i+1)); \
		if [ $$i -ge 50 ]; then \
			pkill -KILL -f 'grafana-conv-app' 2>/dev/null || true; \
			pkill -KILL -f 'target/[d]ebug/grafana-conv' 2>/dev/null || true; \
			pkill -KILL -f '[v]ite' 2>/dev/null || true; \
			break; \
		fi; \
		sleep 0.1; \
	done
	@echo "freed ports $(SEAM_PORT)/$(UI_PORT) and killed any orphaned cargo/vite"
