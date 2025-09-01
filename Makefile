SHELL := bash

# Common bits
ENV_RELEASE := RUST_LOG=info,bevy=info,wgpu_core=warn,wgpu_hal=warn WGPU_BACKEND=metal
LOG_GREP    := grep --line-buffered -E "\[diag\]|\[ffmpeg\]"

# Simple, self-documenting Makefile for common workflows
# Usage: make [target]
# Run `make help` to list available commands

.PHONY: help build run-headless-video run-headless-video-build run-headless-video-build-release run-headless-video-build-memory test fmt clippy clean

# Configurable preview parameters
# Comma-separated list of hosts to send the UDP preview to
UDP_HOST ?= 127.0.0.1
UDP_PORT ?= 12345
VIDEO_FPS ?= 60
# Profiling/signing config
BINARY_NAME := live-whirl
ENTITLEMENTS_FILE ?= $(CURDIR)/ent.plist
XCTRACE_TEMPLATE ?= Allocations
ARGS ?=



help:
	@echo "Available targets:"
	@echo "  build                   - Build the project (debug)"
	@echo "  run-headless-video      - Run headless with video export (writes MP4 via ffmpeg)"
	@echo "  run-headless-video-build- Build then run headless+video with [diag] logs"
	@echo "  run-headless-video-build-release - Build then run headless+video as release"
	@echo "  run-headless-video-build-memory  - Build (release), sign with entitlements, and run Instruments Allocations"
	@echo "  preview-udp             - Preview the UDP stream (env: UDP_HOST (comma-separated)/UDP_PORT/VIDEO_FPS)"
	@echo "  test                    - Run cargo tests"
	@echo "  fmt                     - Format code with rustfmt"
	@echo "  clippy                  - Lint with Clippy (warnings as errors)"
	@echo "  clean                   - Clean target directory"

build:
	cargo build

run-headless-video:
	VIDEO_EXPORT=1 RUST_BACKTRACE=full cargo run

# Target-specific profile selector
run-headless-video-build:           PROFILE :=
run-headless-video-build-release:   PROFILE := --release

# One recipe for both targets
run-headless-video-build run-headless-video-build-release:
	@set -euo pipefail; \
	echo "Build + run headless+video $(if $(PROFILE),release,with diagnostics)"; \
	cargo build $(PROFILE); \
        echo "Launching... UDP hosts $(UDP_HOST) port $(UDP_PORT)"; \
	UDP_HOST="$(UDP_HOST)" UDP_PORT="$(UDP_PORT)" VIDEO_EXPORT=1 \
	$(if $(PROFILE),$(ENV_RELEASE),) \
	RUST_BACKTRACE=full cargo run $(PROFILE) 2>&1 | tee run.log | $(LOG_GREP) || true

test:
	cargo test

fmt:
	cargo fmt --all

# Build (release), codesign with entitlement, and run Instruments Allocations
run-headless-video-build-memory:
	@set -euo pipefail; \
	echo "Building release..."; \
	cargo build --release; \
	BIN_PATH="target/release/$(BINARY_NAME)"; \
	if [ ! -f "$(ENTITLEMENTS_FILE)" ]; then \
	  echo "Creating entitlements at $(ENTITLEMENTS_FILE)"; \
	  printf '%s\n' \
	    '<?xml version="1.0" encoding="UTF-8"?>' \
	    '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">' \
	    '<plist version="1.0">' \
	    '    <dict>' \
	    '        <key>com.apple.security.get-task-allow</key>' \
	    '        <true/>' \
	    '    </dict>' \
	    '</plist>' >"$(ENTITLEMENTS_FILE)"; \
	fi; \
	echo "Codesigning $$BIN_PATH with entitlements"; \
	codesign -s - -f --entitlements "$(ENTITLEMENTS_FILE)" "$$BIN_PATH"; \
	echo "Launching Instruments (template: $(XCTRACE_TEMPLATE))"; \
	xcrun xctrace record --template '$(XCTRACE_TEMPLATE)' --output xctrace-allocations.trace --launch -- "$$BIN_PATH" $(ARGS)


clippy:
	cargo clippy -- -D warnings

clean:
	cargo clean



# Preview UDP stream alongside recording (uses ffplay)
.PHONY: preview-udp
preview-udp:
	# Bind and listen on the local UDP port so ffplay keeps the socket open and
	# automatically resumes when a new ffmpeg process starts sending again.
	# Using udp://@:PORT with listen=1 avoids "connected" UDP semantics that can
	# drop packets from a new sender after restarts.
	ffplay \
	  -flags low_delay \
	  -fflags nobuffer+discardcorrupt+igndts+genpts \
	  -use_wallclock_as_timestamps 1 \
	  -sync video -framedrop \
	  -probesize 32 -analyzeduration 0 \
	  'udp://@:$(UDP_PORT)?listen=1&reuse=1&overrun_nonfatal=1&fifo_size=5000000' \
	  -vf 'fps=$(VIDEO_FPS),setpts=N/($(VIDEO_FPS)*TB)'
