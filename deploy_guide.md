# Pawgloo Bot: Dokploy Deployment Guide

This guide covers the production deployment of the Rust-based Pawgloo Bot using [Dokploy](https://dokploy.com) and the necessary GitHub App configurations.

## 1. Dokploy Setup

### Create a New Application
1. In Dokploy, create a new **Compose** application.
2. Connect your repository: `https://github.com/Pawgloo/bot.git`.
3. Set the branch to `migration/rust` (or your production branch).

### Environment Variables
In the Dokploy **Environment** tab, add the following variables:

| Variable | Value / Source |
| :--- | :--- |
| `GITHUB_APP_ID` | From GitHub App Settings (General) |
| `GITHUB_PRIVATE_KEY_BASE64` | `base64 -i private-key.pem \| tr -d '\n'` |
| `GITHUB_WEBHOOK_SECRET` | Your chosen Webhook Secret |
| `JULES_API_KEY` | From your [Jules](https://jules.google) Settings |
| `JULES_MODE` | `SPEED` or `BALANCED` |
| `OCTOFER_PORT` | `3000` |
| `OCTOFER_HOST` | `0.0.0.0` (Required for Docker) |

> [!IMPORTANT]
> **RSA Key**: Dokploy handles multi-line environment variables, but for safety, copy the **Base64 encoded** string of your [.pem](file:///Users/gaurav/workspace/pawgloo/bot/tests/test_key.pem) key into `GITHUB_PRIVATE_KEY_BASE64`.

---

## 2. GitHub App Configuration

To receive events, your GitHub App must point its webhooks to your Dokploy deployment.

### General Settings
1. **Webhook URL**: `https://your-bot-domain.com/webhook`
   > [!WARNING]
   > Do **NOT** use Dokploy's "Auto Deploy Webhook" URL here. That URL is only for triggering redeploys. Your GitHub App needs the domain where your bot is actually running, followed by `/webhook`.

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

## 4. Resetting Webhook Secret

If you need to reset your `GITHUB_WEBHOOK_SECRET` from scratch:

1.  **Generate**: Run `openssl rand -base64 32` or use a long random string.
2.  **GitHub**: Paste the new secret in **GitHub App Settings → General → Webhook secret**.
3.  **Dokploy**: Update the `GITHUB_WEBHOOK_SECRET` in the **Environment** tab, then **Deploy** to restart.

## 5. Troubleshooting

- **401 Unauthorized**: Mismatch between `GITHUB_WEBHOOK_SECRET` on GitHub vs Dokploy.
- **404 page not found**:
    - **Step 1**: Test direct access: `curl -i http://<VPS_IP>:8000/health`. If this works (200 OK), your bot is fine.
    - **Step 2**: In Dokploy UI → **Application** → **Domains**, ensure the **Service** dropdown is set to **[bot](file:///Users/gaurav/workspace/pawgloo/bot/tests/test_config.rs#64-77)**. Dokploy needs to know which service in the [docker-compose.yml](file:///Users/gaurav/workspace/pawgloo/bot/docker-compose.yml) to route traffic to.
    - **Step 3**: Ensure the domain in Dokploy points to **Port 3000**.
    - **Step 4**: Ensure Cloudflare SSL is set to **"Full"** or **"Full (Strict)"**. "Flexible" will cause 404/500 errors with Dokploy's Traefik proxy.
- **InvalidKeyFormat**: The Base64 encoded key is corrupted or not a valid RSA key.
