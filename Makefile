# Simple, self-documenting Makefile for common workflows
# Usage: make [target]
# Run `make help` to list available commands

.PHONY: help build run-windowed run-headless-video run-headless-video-quiet run-headless-video-log run-headless-video-build test fmt clippy clean

help:
	@echo "Available targets:"
	@echo "  build                   - Build the project (debug)"
	@echo "  run-windowed            - Run in windowed mode (interactive)"
	@echo "  run-headless-video      - Run headless with video export (writes MP4 via ffmpeg)"
	@echo "  run-headless-video-quiet- Same as above but filters to only [diag] logs"
	@echo "  run-headless-video-log  - Same as above; logs to run.log"
	@echo "  run-headless-video-build- Build then run headless+video with [diag] logs"
	@echo "  test                    - Run cargo tests"
	@echo "  fmt                     - Format code with rustfmt"
	@echo "  clippy                  - Lint with Clippy (warnings as errors)"
	@echo "  clean                   - Clean target directory"

build:
	cargo build

run-windowed:
	cargo run

run-headless-video:
	VIDEO_EXPORT=1 cargo run

run-headless-video-quiet:
	@echo "Running headless+video; showing only [diag] lines"
	@VIDEO_EXPORT=1 cargo run 2>&1 | grep -E "\[diag\]|\[ffmpeg\]" || true

# Diagnostics-only variant (stderr to file)
run-headless-video-log:
	@echo "Running headless+video; full logs -> run.log; showing [diag]/[ffmpeg] live"
	@VIDEO_EXPORT=1 cargo run 2>&1 | tee run.log | grep -E "\[diag\]|\[ffmpeg\]" || true

# Build + run in one step using shell operators per request
run-headless-video-build:
	@echo "Build + run headless+video with diagnostics"
	@cargo build && (VIDEO_EXPORT=1 cargo run 2>&1 | tee run.log | grep -E "\[diag\]|\[ffmpeg\]" || true)

test:
	cargo test

fmt:
	cargo fmt --all

clippy:
	cargo clippy -- -D warnings

clean:
	cargo clean

