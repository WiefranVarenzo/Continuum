# ADR-002 — Desktop Local-First Stack

> **Status:** Accepted  
> **Date:** 2026-08-31

## Decision

Use a modular-monolith desktop architecture with Tauri v2, React + TypeScript UI, and Rust application/native core. Windows 11 is the first certified platform; Linux follows through platform adapters. No cloud service is required for canonical local operation.

## Rationale

This preserves a mature UI ecosystem while keeping native filesystem, SQLite, Git, capture, and resource control behind Rust boundaries. It fits the 16 GB target better than a distributed or browser-hosted architecture and maintains replaceable OS/provider adapters.

## Consequences

Typed IPC and strict module contracts are required. Platform capture/credential behavior needs adapters. Team expertise must cover TypeScript and Rust. CP2 establishes the core shell and contracts; later checkpoints add optional adapters.
