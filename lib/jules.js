/**
 * Jules API Integration
 *
 * Uses the Jules REST API (v1alpha) at https://jules.googleapis.com/v1alpha
 * Auth: x-goog-api-key header with JULES_API_KEY
 *
 * Flow:
 *   1. Create a Session with a code-review prompt
 *   2. Poll for session completion
 *   3. List Activities to get the agent's response
 *
 * Because Jules sessions are async (can take minutes),
 * we poll with exponential backoff up to a configurable timeout.
 */

const JULES_BASE = "https://jules.googleapis.com/v1alpha";
const POLL_INITIAL_MS = 3000;
const POLL_MAX_MS = 15000;
const SESSION_TIMEOUT_MS = (process.env.SESSION_TIMEOUT_MINUTES || 25) * 60 * 1000;

export class JulesReviewer {
  constructor(apiKey) {
    if (!apiKey) throw new Error("JULES_API_KEY is required");
    this.apiKey = apiKey;
    this.headers = {
      "Content-Type": "application/json",
      "x-goog-api-key": apiKey,
    };
  }

  /**
   * Sends a code-review request to Jules and returns structured comments.
   * @param {Array<{filename: string, patch: string}>} files
   * @param {{title?: string, body?: string, author?: string, baseBranch?: string, headBranch?: string}} [prMeta]
   * @returns {Promise<{summary: string, comments: Array}>}
   */
  async analyze(files, prMeta = {}) {
    const prompt = this._buildPrompt(files, prMeta);

    try {
      // 1. Create session
      const session = await this._createSession(prompt);
      const sessionId = session.name; // e.g. "sessions/abc123"
      console.log("[Jules] Session created:", sessionId);

      // 2. Poll until done
      const completedSession = await this._pollSession(sessionId);
      console.log("[Jules] Session completed. State:", completedSession.state);
      console.log("[Jules] Session outputs:", JSON.stringify(completedSession.outputs || "none"));

      // 3. Fetch activities (the agent's output)
      const activities = await this._listActivities(sessionId);

      // 4. Parse the agent output into review comments
      return this._parseResponse(activities, files, completedSession);
    } catch (err) {
      console.error("Jules API error:", err.message);
      return {
        summary: `### Jules Code Review\n\n❌ **Error**: ${err.message}`,
        comments: [],
      };
    }
  }

  // ─── Private ──────────────────────────────────────────────

  _buildPrompt(files, prMeta = {}) {
    // Build a structured file list
    const fileList = files.map((f) => `- ${f.filename}`).join("\n");

    const fileContext = files
      .map(
        (f) =>
          `### File: ${f.filename}\n\`\`\`diff\n${f.patch}\n\`\`\``
      )
      .join("\n\n");

    // Include PR metadata for better context
    const prContext = [
      prMeta.title ? `**PR Title**: ${prMeta.title}` : "",
      prMeta.body ? `**PR Description**: ${prMeta.body}` : "",
      prMeta.author ? `**Author**: ${prMeta.author}` : "",
      prMeta.baseBranch && prMeta.headBranch
        ? `**Branch**: ${prMeta.headBranch} → ${prMeta.baseBranch}`
        : "",
    ]
      .filter(Boolean)
      .join("\n");

    return `You are a Senior Code Reviewer performing an automated review of a Pull Request.

${prContext ? `## Pull Request Context\n${prContext}\n` : ""}
## Changed Files
${fileList}

## Review Instructions

Analyze each file diff below. Focus on issues that matter — skip formatting nitpicks.

**Priority areas:**
1. **Security** – SQL injection, XSS, secrets in code, unsafe deserialization, auth bypass
2. **Logic errors** – off-by-one, null/undefined dereference, race conditions, unhandled edge cases
3. **Performance** – unnecessary loops, missing indexes, memory leaks, N+1 queries
4. **Clean code** – unclear naming, excessive complexity, code duplication (only if impactful)

**Do NOT flag:**
- Minor style preferences (spacing, bracket style)
- Import ordering
- Trivial naming suggestions unless genuinely confusing

For each issue found, respond with a JSON object. Each element in the "issues" array must have:
- "file": the filename exactly as shown
- "line": the line number in the NEW file (right side of the diff) where the issue is
- "severity": "critical" | "warning" | "suggestion"
- "comment": a concise explanation of the issue and how to fix it

If everything looks good, return an empty issues array.

Respond ONLY with valid JSON in this format:
{
  "summary": "High-level overview of the PR quality and any key concerns",
  "issues": [ { "file": "...", "line": 10, "severity": "warning", "comment": "..." } ]
}

## Diffs

${fileContext}`;
  }

  async _createSession(prompt) {
    const res = await fetch(`${JULES_BASE}/sessions`, {
      method: "POST",
      headers: this.headers,
      body: JSON.stringify({
        prompt: prompt,
        title: "Automated Code Review",
      }),
    });

    if (!res.ok) {
      const body = await res.text();
      throw new Error(`Failed to create Jules session: ${res.status} ${body}`);
    }

    return res.json();
  }

  async _pollSession(sessionId) {
    const start = Date.now();
    let delay = POLL_INITIAL_MS;

    while (Date.now() - start < SESSION_TIMEOUT_MS) {
      await this._sleep(delay);

      const res = await fetch(`${JULES_BASE}/${sessionId}`, {
        headers: this.headers,
      });

      if (!res.ok) {
        const body = await res.text();
        throw new Error(`Failed to poll session: ${res.status} ${body}`);
      }

      const session = await res.json();

      if (
        session.state === "COMPLETED" ||
        session.state === "FAILED" ||
        session.state === "CANCELLED"
      ) {
        if (session.state !== "COMPLETED") {
          throw new Error(`Jules session ended with state: ${session.state}`);
        }
        return session;
      }

      // Exponential backoff
      delay = Math.min(delay * 1.5, POLL_MAX_MS);
    }

    throw new Error(`Jules session timed out after ${SESSION_TIMEOUT_MS / 60000} minutes`);
  }

  async _listActivities(sessionId) {
    const res = await fetch(`${JULES_BASE}/${sessionId}/activities?pageSize=30`, {
      headers: this.headers,
    });

    if (!res.ok) {
      const body = await res.text();
      throw new Error(`Failed to list activities: ${res.status} ${body}`);
    }

    return res.json();
  }

  _parseResponse(activitiesResponse, files, completedSession = null) {
    // Debug: log the entire raw activities response so we can diagnose field names
    console.log(
      "[Jules] Raw activities response:",
      JSON.stringify(activitiesResponse, null, 2)
    );

    const activities = activitiesResponse.activities || activitiesResponse || [];
    let rawText = "";

    // Try multiple possible field names for agent responses
    for (const activity of activities) {
      console.log("[Jules] Activity keys:", Object.keys(activity));

      // Check all known and possible field names
      // NOTE: Jules API uses "agentMessaged" (with 'd') as the wrapper,
      //       and "agentMessage" inside it contains the actual text.
      const msg =
        activity.agentMessaged?.agentMessage ||
        activity.agentMessaged ||
        activity.agentMessage ||
        activity.agent_message ||
        activity.message ||
        activity.response ||
        activity.output ||
        activity.text ||
        activity.content ||
        null;

      if (msg) {
        if (typeof msg === "string") {
          rawText = msg;
        } else {
          rawText =
            msg.text ||
            msg.content ||
            msg.message ||
            msg.body ||
            msg.response ||
            (typeof msg === "object" ? JSON.stringify(msg) : String(msg));
        }
        console.log("[Jules] Found agent text in activity:", rawText.substring(0, 200));
      }

      // Also check for nested textContent or parts (Gemini-style)
      if (activity.parts) {
        for (const part of activity.parts) {
          if (part.text) {
            rawText = part.text;
            console.log("[Jules] Found text in activity.parts:", rawText.substring(0, 200));
          }
        }
      }
    }

    // Fallback: check session outputs (Jules API can embed results here)
    if (!rawText && completedSession?.outputs) {
      console.log("[Jules] Checking completedSession.outputs:", JSON.stringify(completedSession.outputs));
      for (const output of completedSession.outputs) {
        const text = output.text || output.content || output.message || output.response;
        if (text) {
          rawText = typeof text === "string" ? text : JSON.stringify(text);
          console.log("[Jules] Found text in session outputs:", rawText.substring(0, 200));
        }
      }
    }

    // If still nothing found, try to extract text from the entire response
    if (!rawText && activitiesResponse) {
      const fullStr = JSON.stringify(activitiesResponse);
      // Look for our expected JSON structure anywhere in the response
      const jsonMatch = fullStr.match(
        /\{[^{}]*"summary"\s*:\s*"[^"]*"[^{}]*"issues"\s*:\s*\[[\s\S]*?\]\s*\}/
      );
      if (jsonMatch) {
        rawText = jsonMatch[0];
        console.log("[Jules] Extracted JSON from raw response via regex");
      }
    }

    if (!rawText) {
      console.error(
        "[Jules] Could not find agent response in activities. Full response:",
        JSON.stringify(activitiesResponse)
      );
      return {
        summary: "### Jules Code Review\n\nNo response received from Jules.",
        comments: [],
      };
    }

    // Try to extract JSON from the response
    try {
      // Strip markdown code fences if present
      let jsonStr = rawText
        .replace(/```json\n?/g, "")
        .replace(/```\n?/g, "")
        .trim();

      // If the response contains JSON embedded in other text, try to extract it
      const jsonMatch = jsonStr.match(/\{[\s\S]*"summary"[\s\S]*"issues"[\s\S]*\}/);
      if (jsonMatch) {
        jsonStr = jsonMatch[0];
      }

      const parsed = JSON.parse(jsonStr);
      const comments = [];
      const validFiles = new Set(files.map((f) => f.filename));

      if (parsed.issues && Array.isArray(parsed.issues)) {
        for (const issue of parsed.issues) {
          if (validFiles.has(issue.file) && issue.line && issue.comment) {
            const emoji =
              issue.severity === "critical"
                ? "🔴"
                : issue.severity === "warning"
                ? "🟡"
                : "🔵";

            comments.push({
              path: issue.file,
              line: issue.line,
              side: "RIGHT",
              body: `${emoji} **${issue.severity?.toUpperCase() || "NOTE"}**: ${issue.comment}`,
            });
          }
        }
      }

      const summary =
        parsed.summary ||
        (comments.length === 0
          ? "LGTM 👍 — No issues found."
          : `Found ${comments.length} issue(s).`);

      return {
        summary: `### Jules Code Review\n\n${summary}`,
        comments,
      };
    } catch {
      // If we can't parse JSON, just post the raw text as a summary
      return {
        summary: `### Jules Code Review\n\n${rawText}`,
        comments: [],
      };
    }
  }

  _sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}

