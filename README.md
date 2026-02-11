# bot

> A GitHub App built with [Probot](https://github.com/probot/probot) that reviews Pull Requests using [Google Jules](https://jules.google/).

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
| Manual | Comment `/pawgloo-review` on any PR |

## Docker

```sh
docker build -t bot .
docker run -e APP_ID=<id> -e PRIVATE_KEY=<pem> -e WEBHOOK_SECRET=<secret> -e JULES_API_KEY=<key> bot
```

## License

[ISC](LICENSE) © 2026 gaurav2361
