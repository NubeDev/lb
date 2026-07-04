# lazybones app/ — developer tasks for the React Native shell + its browser preview.
#
# The app is bare React Native (RN 0.86 + Re.Pack) with Expo's BARE modules adopted (SDK 57 —
# expo-modules-core + expo-* like expo-secure-store; NOT the managed workflow — Re.Pack stays the one
# bundler, never `expo start`/`expo prebuild`; see shell/README.md). Shared @nube/app-sdk client.
# There are two ways to RUN it:
#   web       the react-native-web PREVIEW — the SAME App.tsx + screens in a browser on this PC,
#             no emulator (react-native-web via vite.config.web.mts). The fast UI-iteration loop.
#   android   / ios — a real device build (needs the Android SDK / Xcode; see shell/README.md).
#
# The preview talks to a REAL gateway node (the `test_gateway` bin), exactly like the app's tests.
# TL;DR:
#   make setup     one-time: standalone pnpm install in shell/ + sdk/
#   make gateway   run the real gateway node on GW_PORT (the preview points at it)
#   make web       run the browser preview on WEB_PORT
#   make dev       gateway + web preview together (Ctrl-C stops both)
#   make kill      free the preview ports if a previous run was left running
#   make test      the sdk's real-gateway suite (spawns its own node)
#   make pods      (macOS) CocoaPods install — REQUIRED before `make ios` after a native/dep change
#   make ios / android  real device builds (ios runs pods first)

HERE     := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
REPO     := $(abspath $(HERE)..)
RUST     := $(REPO)/rust
SHELL_DIR := $(HERE)shell
SDK_DIR  := $(HERE)sdk

# The gateway the preview points at, and the vite preview server port (must match
# vite.config.web.mts `server.port`). Override on the command line if they collide.
# GW_PORT is deliberately NOT 8080: that's the root Makefile's dev node (persistent store,
# possibly owned by another terminal). The preview gets its own throwaway node on its own port.
GW_HOST  ?= 127.0.0.1
GW_PORT  ?= 8087
GW_URL   := http://$(GW_HOST):$(GW_PORT)
WEB_PORT ?= 5310
WEB_URL  := http://127.0.0.1:$(WEB_PORT)

# The real gateway harness binary (same one app/sdk tests spawn). Built on demand by `gateway`.
GW_BIN   := $(RUST)/target/debug/test_gateway

# Android SDK location for the `android` target. The RN CLI shells out to `adb`/`emulator`, so they
# must be on PATH — but a fresh machine often has the SDK installed (Android Studio's default
# ~/Android/Sdk) without ever exporting ANDROID_HOME. Resolve it here so `make android` works without
# the developer editing their shell profile. Honors an existing ANDROID_HOME/ANDROID_SDK_ROOT; else
# falls back to the platform-default install dir. Override on the CLI: make android ANDROID_HOME=/path.
ANDROID_HOME ?= $(or $(ANDROID_SDK_ROOT),$(HOME)/Android/Sdk)

# On a PHYSICAL device `127.0.0.1` is the phone, not this PC, so the app can't reach a
# gateway on the dev box by localhost. The RN CLI already `adb reverse`s Metro's 8081;
# we additionally reverse the gateway port so the shell's default node URL
# (http://127.0.0.1:$(ANDROID_NODE_PORT)) tunnels back to the gateway running here.
# Override to match your node: make android ANDROID_NODE_PORT=8087.
ANDROID_NODE_PORT ?= 8080
ADB := $(ANDROID_HOME)/platform-tools/adb

.PHONY: setup gateway web dev android ios pods test typecheck kill

## one-time: standalone installs (shell/ is deliberately OUTSIDE the pnpm workspace — see .npmrc).
setup:
	cd $(SDK_DIR) && pnpm install
	cd $(SHELL_DIR) && pnpm install --ignore-workspace

## build (if needed) and run the REAL gateway node the preview/tests talk to. The store is
## IN-MEMORY (test_gateway sets no LB_STORE_PATH → Store::memory in host boot), so every boot is
## fresh by construction and the prefilled dev login (ada/acme) always bootstraps cleanly — a
## workspace is owned by whoever logs in FIRST (global-identity decision #4). If ada/acme ever
## 403s "not a member", the cause is a STALE test_gateway still holding $(GW_PORT) (the new one
## fails to bind and exits, so you keep talking to the old node's memory) — which is why we free
## the port first. LB_DIR below only holds node keys/config, not the store.
gateway:
	cd $(RUST) && cargo build -p lb-role-gateway --features test-harness --bin test_gateway
	@pid=$$(fuser $(GW_PORT)/tcp 2>/dev/null | tr -d ' '); \
	if [ -n "$$pid" ]; then \
		exe=$$(readlink /proc/$$pid/exe 2>/dev/null); \
		case "$$exe" in \
		*test_gateway) \
			kill -TERM $$pid 2>/dev/null; i=0; \
			while kill -0 $$pid 2>/dev/null; do \
				i=$$((i+1)); \
				if [ $$i -ge 50 ]; then kill -KILL $$pid 2>/dev/null; break; fi; \
				sleep 0.1; \
			done ;; \
		*) \
			echo "ERROR: port $(GW_PORT) is held by PID $$pid ($$exe) — NOT a test_gateway."; \
			echo "That's someone else's node (e.g. the root 'make dev' node). Refusing to kill it."; \
			echo "Run with another port: make gateway GW_PORT=<free-port>"; \
			exit 1 ;; \
		esac; \
	fi
	PORT=$(GW_PORT) \
	LB_DEVKIT_ROOT=$(RUST)/extensions \
	LB_DIR=$(RUST)/target/app-preview-lb \
	$(GW_BIN)

## the react-native-web preview (browser). Point a browser at $(WEB_URL)/?node=$(GW_URL).
## NOTE: HMR reflects edits live, but ADDING/REMOVING a module export (vite caches a `?t=` copy)
## can serve a stale module → "does not provide an export named X" + a blank root. Restart `make web`
## (or `rm -rf shell/node_modules/.vite`) after an export change; HMR alone won't pick it up.
web:
	@echo "preview → $(WEB_URL)/?node=$(GW_URL)"
	cd $(SHELL_DIR) && pnpm web

## gateway + web preview together; Ctrl-C stops both (kills the whole process group on exit).
dev:
	@$(MAKE) -C $(HERE:%/=%) --no-print-directory _dev

_dev:
	@echo "starting gateway ($(GW_URL)) + web preview ($(WEB_URL)) — Ctrl-C stops both"
	@trap '$(MAKE) -f $(HERE)Makefile kill' INT TERM EXIT; \
	( $(MAKE) -f $(HERE)Makefile gateway & \
	  $(MAKE) -f $(HERE)Makefile web & \
	  wait )

## CocoaPods install (macOS only). REQUIRED after adopting/changing a native dep — the Expo bare
## modules (expo-secure-store, …) autolink as pods through `use_expo_modules!` in ios/Podfile, so the
## pod set must be regenerated before the next `make ios`. Runs through Bundler (the pinned Gemfile
## CocoaPods) so the pod version matches. Skips cleanly on non-macOS (no ios/ pod project to install).
pods:
	@if [ "$$(uname)" != "Darwin" ]; then \
		echo "make pods: iOS/CocoaPods is macOS-only — skipping (nothing to do on $$(uname))."; \
	else \
		cd $(SHELL_DIR) && bundle install && cd ios && bundle exec pod install; \
	fi

## real device builds (need the native toolchain — see shell/README.md). `ios` installs pods first so
## the Expo bare modules are linked; `android` autolinks them through Gradle at build time (no extra step).
android:
	@if [ ! -x "$(ANDROID_HOME)/platform-tools/adb" ]; then \
		echo "make android: no adb at $(ANDROID_HOME)/platform-tools — set ANDROID_HOME to your SDK."; \
		echo "  (install via Android Studio, or: make android ANDROID_HOME=/path/to/Android/Sdk)"; \
		exit 1; \
	fi
	cd $(SHELL_DIR) && \
		ANDROID_HOME="$(ANDROID_HOME)" \
		PATH="$(ANDROID_HOME)/platform-tools:$(ANDROID_HOME)/emulator:$$PATH" \
		pnpm android
	@# Tunnel the gateway port back to this PC so the device's 127.0.0.1:<port> reaches it.
	@$(ADB) reverse tcp:$(ANDROID_NODE_PORT) tcp:$(ANDROID_NODE_PORT) >/dev/null 2>&1 \
		&& echo "adb reverse tcp:$(ANDROID_NODE_PORT) → gateway on this host" \
		|| echo "note: 'adb reverse tcp:$(ANDROID_NODE_PORT)' failed (no device yet?) — rerun once the device is listed"
ios: pods
	cd $(SHELL_DIR) && pnpm ios

## the sdk's real-gateway test suite (spawns its own throwaway node; no `make gateway` needed).
test:
	cd $(SDK_DIR) && pnpm test:gateway

## typecheck both packages.
typecheck:
	cd $(SDK_DIR) && pnpm typecheck
	cd $(SHELL_DIR) && pnpm typecheck

# Free the preview ports and reap any orphaned gateway/vite. Mirrors the root Makefile's `kill`:
# SIGTERM first, then poll pgrep and escalate stragglers to SIGKILL after ~5s. The wait is both the
# graceful-shutdown grace period AND the race-closer so a returning `make kill` immediately followed
# by `make dev` doesn't fight the dying process for the port. We match the test_gateway binary and
# ONLY the app-shell vite (vite.config.web.mts) by pattern — a bare '[v]ite' would also kill the
# web shell's ui/ vite from another terminal. (Bracketed first char so pgrep/pkill don't match
# themselves.)
kill:
	-@fuser -TERM -k $(GW_PORT)/tcp 2>/dev/null || true
	-@fuser -TERM -k $(WEB_PORT)/tcp 2>/dev/null || true
	-@pkill -TERM -f '[t]est_gateway' 2>/dev/null || true
	-@pkill -TERM -f '[v]ite.*vite.config.web.mts' 2>/dev/null || true
	@i=0; \
	while pgrep -f '[t]est_gateway' >/dev/null 2>&1 \
	   || pgrep -f '[v]ite.*vite.config.web.mts' >/dev/null 2>&1; do \
		i=$$((i+1)); \
		if [ $$i -ge 50 ]; then \
			pkill -KILL -f '[t]est_gateway' 2>/dev/null || true; \
			pkill -KILL -f '[v]ite.*vite.config.web.mts' 2>/dev/null || true; \
			break; \
		fi; \
		sleep 0.1; \
	done
	@echo "freed ports $(GW_PORT)/$(WEB_PORT) and killed any orphaned test_gateway/vite"
