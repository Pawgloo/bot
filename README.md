# bot

> An intelligent GitHub App that acts as a Senior Code Reviewer. It automatically reviews Pull Requests using **Google Jules**, focusing on security, logic, and clean code principles. Trigger it automatically on PRs or manually with `/pawgloo-review`.

## Setup

```sh
# Install dependencies
npm install

# Run the bot
npm start
```

## Configuration

Copy `.env.example` to `.env` and fill in:

| Variable | Description |
|---|---|
| `APP_ID` | Your GitHub App ID |
| `PRIVATE_KEY` | Your GitHub App private key |
| `WEBHOOK_SECRET` | Webhook secret (set on GitHub App settings) |
| `WEBHOOK_PROXY_URL` | Smee.io URL for local dev |
| `JULES_API_KEY` | API key from [jules.google](https://jules.google) → Settings |
| `IGNORE_PATTERNS` | *(Optional)* Comma-separated globs to skip (default: `docs/,*.md,...`) |
| `MAX_PATCH_LENGTH` | *(Optional)* Max chars per file patch before skipping |

## Triggers

| Trigger | How |
|---|---|
| Automatic | Opens a PR or pushes new commits |
| Automatic | Opens a PR or pushes new commits |
| Manual | Comment `/pawgloo-review` on any PR (works on old PRs too!) |

## How it works

The bot listens for PR events and sends the code diff to **Google Jules**.

### The Prompt
It acts as a **Senior Code Reviewer** with the following focus areas:
1. **Security** – SQL injection, XSS, secrets in code, unsafe deserialization
2. **Logic errors** – off-by-one, null pointer, race conditions
3. **Clean code** – naming, complexity, duplication
4. **Performance** – unnecessary loops, missing indexes, memory leaks

It then posts a **Review** on the PR with inline comments for specific lines and a high-level summary.

## Docker

```sh
docker build -t bot .
docker run -e APP_ID=<id> -e PRIVATE_KEY=<pem> -e WEBHOOK_SECRET=<secret> -e JULES_API_KEY=<key> bot
```

## License

[ISC](LICENSE) © 2026 gaurav2361
