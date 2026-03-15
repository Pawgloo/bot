# Pawgloo Bot: Rust Migration Handoff Context

This document provides a complete summary of the migration from Node.js (Probot) to Rust (octofer) to ensure continuity for future development or AI agents.

## Project Overview

The **Pawgloo Bot** is a GitHub App designed to automate Pull Request reviews using the **Jules AI API**. It triggers automatically on PR events or manually via `/pawgloo` comments.

## Migration Summary

The core logic has been moved from a Node.js/TypeScript environment to a performance-oriented Rust codebase using the standard `octofer` framework.

### 1. Key Components

- **[src/lib.rs](file:///Users/gaurav/workspace/pawgloo/bot/src/lib.rs)**: Core library containing app orchestration and state management.
- **[src/main.rs](file:///Users/gaurav/workspace/pawgloo/bot/src/main.rs)**: Thin binary entry point.
- **[src/handlers.rs](file:///Users/gaurav/workspace/pawgloo/bot/src/handlers.rs)**: GitHub event handlers (PR Opened/Sync, Issue Comment).
- **[src/jules.rs](file:///Users/gaurav/workspace/pawgloo/bot/src/jules.rs)**: Jules AI API client (session management, polling, result parsing).
- **[src/review.rs](file:///Users/gaurav/workspace/pawgloo/bot/src/review.rs)**: The review pipeline (fetching diffs, prompt engineering, comment posting).
- **[src/config.rs](file:///Users/gaurav/workspace/pawgloo/bot/src/config.rs)**: Environment-based configuration.

### 2. Implemented Features

- [x] **Auto-Trigger**: Evaluates PRs on `opened` and `synchronize`.
- [x] **Manual Commands**: Trigger via `/pawgloo` or `/pawgloo-review`.
- [x] **Smart Filtering**: Ignores files based on glob patterns and type (e.g., locks, images).
- [x] **Advanced Prompting**: Uses a "Senior Adversarial Reviewer" persona with Big O analysis and STRIDE security checks.
- [x] **E2E Testing**: Local bash script ([tests/run_e2e_tests.sh](file:///Users/gaurav/workspace/pawgloo/bot/tests/run_e2e_tests.sh)) to simulate webhooks without external dependencies.
- [x] **CI/CD**: GitHub Actions workflow and Docker multi-stage build configuration.

## Technical Context for Future Work

### Environment Configuration ([.env](file:///Users/gaurav/workspace/pawgloo/bot/.env))

The bot expects the following variables:
| Variable | Description |
|----------|-------------|
| `GITHUB_APP_ID` | GitHub App ID (numeric). |
| `GITHUB_PRIVATE_KEY_BASE64` | Base64 encoded RSA PEM key (PKCS#1 or PKCS#8). |
| `JULES_API_KEY` | Jules API authentication key. |
| `GITHUB_WEBHOOK_SECRET` | Secret for validating GitHub signatures. |
| `OCTOFER_PORT` | The local port to listen on (default 8000). |

> [!TIP]
> **Private Key Format**: Using `GITHUB_PRIVATE_KEY_BASE64` is recommended for [.env](file:///Users/gaurav/workspace/pawgloo/bot/.env) files to avoid newline escaping issues. You can generate it via `base64 -i key.pem | tr -d '\n'`.

### Critical Implementation Details

- **Error Handling**: Uses `thiserror` for internal errors and `anyhow` for top-level propagation.
- **Safety**: Follows [AGENTS.md](file:///Users/gaurav/workspace/pawgloo/bot/AGENTS.md) (Rust Best Practices) strictly (ownership, memory optimization).
- **Testing**:
  - `cargo test`: Runs unit tests for diff parsing and filtering.
  - [./tests/run_e2e_tests.sh](file:///Users/gaurav/workspace/pawgloo/bot/tests/run_e2e_tests.sh): Runs integration tests against a live local server.

## Fixed Startup Issues

- **InvalidKeyFormat / Missing Variables**: Identified that `octofer` requires `GITHUB_` prefixes (e.g., `GITHUB_APP_ID`) instead of generic `APP_ID`. These have been updated in [README.md](file:///Users/gaurav/workspace/pawgloo/bot/README.md) and [.env.example](file:///Users/gaurav/workspace/pawgloo/bot/.env.example).
- **Webhook Route**: The framework hardcodes the webhook listener at `/webhook`. Ensure your GitHub App Settings reflect this.
- **Linker Error (`-liconv`)**: On macOS, always run with:
  `LIBRARY_PATH="$(xcode-select -p)/Toolchains/XcodeDefault.xctoolchain/usr/lib:$(xcrun --show-sdk-path)/usr/lib" cargo run`

---

_Created by Antigravity AI - 2026-03-15_
