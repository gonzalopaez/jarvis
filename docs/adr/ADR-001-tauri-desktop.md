# ADR-001: Tauri for Desktop

- Status: Accepted
- Date: 2026-08-08

## Decision

Use Tauri for the Linux-first Desktop. The web UI provides the approved HUD while Rust exposes narrowly scoped native capabilities such as local telemetry.

## Consequences

Tauri commands and permissions are deny-by-default and minimal. Desktop has no direct infrastructure, provider, secret-store or shell access. New native capabilities require threat review, typed contracts and tests.
