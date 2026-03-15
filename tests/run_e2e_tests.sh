#!/bin/bash
# A local integration test script to send mock webhook payloads to learning bot.
# Ensure `cargo run` is actively running listening on port 3000

BOT_URL="http://127.0.0.1:3000"

echo "Running E2E Mock Tests against Pawgloo Bot at ${BOT_URL}"
echo "--------------------------------------------------------"

# Wait for bot to be healthy
if ! curl -s "${BOT_URL}/api/github/webhooks" > /dev/null; then
    echo "⚠️  Error: Could not reach bot at ${BOT_URL}"
    echo "Make sure the bot is running via 'cargo run'."
    exit 1
fi

echo "✅ Bot is reachable."

echo -e "\n1️⃣  Sending 'pull_request' (action: opened) event..."
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${BOT_URL}/api/github/webhooks" \
     -H "Content-Type: application/json" \
     -H "X-GitHub-Event: pull_request" \
     -d @tests/payloads/pr_opened.json)

if [ "$RESPONSE" -eq 200 ] || [ "$RESPONSE" -eq 202 ]; then
    echo "✅ PR Opened Webhook accepted by bot (HTTP $RESPONSE)"
else
    echo "❌ Failed: Bot returned HTTP $RESPONSE"
    exit 1
fi

echo -e "\n2️⃣  Sending 'issue_comment' (action: created, /pawgloo-review) event..."
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${BOT_URL}/api/github/webhooks" \
     -H "Content-Type: application/json" \
     -H "X-GitHub-Event: issue_comment" \
     -d @tests/payloads/issue_comment.json)

if [ "$RESPONSE" -eq 200 ] || [ "$RESPONSE" -eq 202 ]; then
    echo "✅ Issue Comment Webhook accepted by bot (HTTP $RESPONSE)"
else
    echo "❌ Failed: Bot returned HTTP $RESPONSE"
    exit 1
fi

echo -e "\n🎉 All local webhook E2E deliveries successful."
echo "Check the bot's terminal output to verify tracing logs (e.g. 'Auto-trigger fired', 'Manual trigger received', 'installation client available' etc...)"
