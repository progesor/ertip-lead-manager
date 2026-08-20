# ADR 0001 — Windows desktop stack

**Status:** Accepted for V1 bootstrap  
**Decision:** Tauri 2 + React + TypeScript + Rust

## Context

The product is Windows-first, offline-capable, data-table/dashboard heavy, and expected to remain relatively lightweight. Web UI technologies enable fast design iteration while Tauri supplies native packaging/file access with a smaller runtime footprint than a traditional Electron bundle.

## Decision

Use Tauri 2 for desktop shell and native bridge, React/TypeScript/Vite for UI, Rust for domain/persistence/import logic.

## Consequences

Positive:

- native desktop packaging;
- strong Rust boundary for filesystem/SQLite/Excel parsing;
- flexible UI ecosystem;
- future shared web-style components possible.

Costs:

- Rust + TypeScript dual-language codebase;
- Tauri command boundary requires DTO discipline;
- Windows build environment/toolchain setup.
