# Pawgloo Bot: Dokploy Deployment Guide

This guide covers the production deployment of the Rust-based Pawgloo Bot using [Dokploy](https://dokploy.com) and the necessary GitHub App configurations.

## 1. Dokploy Setup

### Create a New Application

1. In Dokploy, create a new **Compose** application.
2. Connect your repository: `https://github.com/Pawgloo/bot.git`.
3. Set the branch to `migration/rust` (or your production branch).

### Environment Variables

In the Dokploy **Environment** tab, add the following variables:

| Variable                    | Value / Source                                   |
| :-------------------------- | :----------------------------------------------- |
| `GITHUB_APP_ID`             | From GitHub App Settings (General)               |
| `GITHUB_PRIVATE_KEY_BASE64` | `base64 -i private-key.pem \| tr -d '\n'`        |
| `GITHUB_WEBHOOK_SECRET`     | Your chosen Webhook Secret                       |
| `JULES_API_KEY`             | From your [Jules](https://jules.google) Settings |
| `JULES_MODE`                | `SPEED` or `BALANCED`                            |
| `OCTOFER_PORT`              | `3000`                                           |

> [!IMPORTANT]
> **RSA Key**: Dokploy handles multi-line environment variables, but for safety, copy the **Base64 encoded** string of your `.pem` key into `GITHUB_PRIVATE_KEY_BASE64`.

---

## 2. GitHub App Configuration

To receive events, your GitHub App must point its webhooks to your Dokploy deployment.

### General Settings

1. **Webhook URL**: `https://your-dokploy-domain.com/webhook`
   - _Note: Ensure the `/webhook` suffix is present._
2. **Webhook Secret**: Must match the `GITHUB_WEBHOOK_SECRET` in Dokploy.

### Permissions & Events

Ensure the following permissions are active under **Permissions & events**:

- **Pull Requests**: `Read & write`
  - [x] Check: `Opened`, `Synchronize`, `Reopened`
- **Issues / Issue Comments**: `Read & write`
  - [x] Check: `Created`
- **Metadata**: `Read-only` (Automatic)

---

## 3. Verification

Once deployed:

1. **Health Check**: Visit `https://your-dokploy-domain.com/health`. You should see a blank page with a `200 OK` status.
2. **Test Event**: Go to GitHub App Settings → **Advanced**.
   - Find a recent `ping` or [pull_request](file:///Users/gaurav/workspace/pawgloo/bot/octofer/src/events/prs.rs#13-120) event.
   - Click **Redeliver**.
   - Dokploy logs should show: `2026-03-15... INFO 🤖 Event received: pull_request`.

## 4. Troubleshooting

- **401 Unauthorized**: Mismatch between `GITHUB_WEBHOOK_SECRET` on GitHub vs Dokploy.
- **404 Not Found**: You likely pointed GitHub to `/api/github/webhooks` instead of just `/webhook`.
- **InvalidKeyFormat**: The Base64 encoded key is corrupted or not a valid RSA key.
