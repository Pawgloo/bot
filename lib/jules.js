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
const SESSION_TIMEOUT_MS =
	(process.env.SESSION_TIMEOUT_MINUTES || 25) * 60 * 1000;

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
			console.log(
				"[Jules] Session outputs:",
				JSON.stringify(completedSession.outputs || "none"),
			);

			// 3. Fetch activities (the agent's output)
			const activities = await this._listActivities(sessionId);

			// 4. Parse the agent output into review comments
			return this._parseResponse(activities, files, completedSession);
		} catch (err) {
			console.error("Jules API error:", err.message);
			return {
				summary: `### Code Review\n\n❌ **Error**: ${err.message}`,
				comments: [],
			};
		}
	}

	// ─── Private ──────────────────────────────────────────────

	_buildPrompt(files, prMeta = {}) {
		// Build a structured file list
		const fileList = files.map((f) => `- \`${f.filename}\``).join("\n");

		const fileContext = files
			.map((f) => `### File: \`${f.filename}\`\n\`\`\`diff\n${f.patch}\n\`\`\``)
			.join("\n\n");

		// Include PR metadata for better context
		const prContext = [
			prMeta.title ? `**PR Title**: ${prMeta.title}` : "",
			prMeta.body ? `**PR Description**: ${prMeta.body}` : "",
			prMeta.author ? `**Author**: ${prMeta.author}` : "",
			prMeta.baseBranch && prMeta.headBranch
				? `**Branch**: \`${prMeta.headBranch}\` → \`${prMeta.baseBranch}\``
				: "",
		]
			.filter(Boolean)
			.join("\n");

		// Detect file extensions for language-aware instructions
		const extensions = [
			...new Set(files.map((f) => f.filename.split(".").pop())),
		];
		const langHint = extensions.join(", ");

		return `Role: You are a fiercely pragmatic Principal Engineer and a paranoid Security Researcher. You have zero tolerance for over-engineering, unhandled edge cases, and security vulnerabilities. Review this code with brutal technical precision. Do NOT soften your critique. Do NOT praise mediocre code. If the code is genuinely good, say so in one sentence and move on — do not pad your response with compliments. Your job is to find real problems that would cause production outages, security breaches, or maintenance nightmares.

${prContext ? `## Pull Request Context\n${prContext}\n` : ""}
## Changed Files (${langHint})
${fileList}

---

## Review Methodology

Analyze each file diff below systematically. Apply the following frameworks:

### 1. SECURITY (STRIDE Threat Model)
- **Spoofing**: Authentication bypass, weak session management, token validation gaps.
- **Tampering**: Mass assignment, parameter pollution, insecure deserialization, XSS.
- **Information Disclosure**: Hardcoded secrets, leaked PII in logs/errors, verbose stack traces.
- **Elevation of Privilege**: IDOR, missing RBAC checks, dev backdoors, self-privilege escalation.
- For each security issue, provide an **attack narrative** — explain exactly how a threat actor would exploit it.

### 2. LOGIC (Correctness & Edge Cases)
- Bugs, off-by-one errors, null/undefined dereference, race conditions.
- Unhandled edge cases: What happens at n-1, n, n+1? At empty input? At max capacity?
- Incorrect control flow, unreachable code, silent failures.

### 3. CLEAN CODE (DRY, SOLID, Performance)
- DRY violations: duplicated logic that should be abstracted.
- SOLID infractions: God classes, tight coupling, missing dependency injection.
- Performance: Identify N+1 queries, memory leaks, or unclosed resources. You MUST explicitly state the Time (Big O) and Space complexity of the current implementation versus your proposed optimization before suggesting any performance change.
- YAGNI: Speculative "future-proofing" code that adds complexity for no current requirement.
- **Dependency Constraint**: All suggested fixes must use ONLY native language features. You are strictly FORBIDDEN from introducing new external dependencies or third-party libraries. Keep fixes backward compatible.

### What NOT to flag
- Minor style preferences (spacing, bracket style, trailing commas)
- Import ordering
- Trivial naming nitpicks unless genuinely confusing
- Do NOT praise code that is merely adequate

---

## Output Format

Respond ONLY with valid JSON. No text before or after the JSON object.

### "analysis_scratchpad" field — Chain-of-Thought Reasoning

Before generating the summary and issues, you MUST think step-by-step in the \`"analysis_scratchpad"\` field. Use it to:
- Trace data flow and trust boundaries across the changed files
- Identify where untrusted input enters and how it propagates
- Evaluate the structural integrity and coupling of the code
- Reason about edge cases, concurrency, and failure modes

This is your internal reasoning space. It will not be shown to the user but it forces you to reason properly before generating findings.

### "summary" field — Detailed PR Summary (Markdown)

Write a comprehensive PR summary with these sections:

1. **Purpose & Scope**: What this PR does, what problem it solves, files/modules affected.
2. **Architecture**: Key patterns, data flow, component interactions.
   - If the PR involves complex architecture, include a Mermaid diagram inside a fenced code block like this:
   \`\`\`mermaid
   graph TD
       A[Component] --> B[Component]
   \`\`\`
3. **Code Quality**: Overall assessment — structure, readability, maintainability.
4. **Risks & Concerns**: Security vulnerabilities, logic gaps, technical debt introduced.
5. **Verdict**: One of: **Approve**, **Request Changes**, or **Needs Discussion** — with a one-line justification.

### "issues" array — Inline File Comments

Each issue object must have:
- \`"file"\`: filename exactly as shown
- \`"line"\`: line number in the NEW file (right side of diff)
- \`"severity"\`: \`"critical"\` | \`"warning"\` | \`"suggestion"\`
- \`"category"\`: \`"SECURITY"\` | \`"LOGIC"\` | \`"CLEAN CODE"\`
- \`"comment"\`: explanation + code suggestion

**Comment formatting rules (STRICT — violations will break rendering):**
1. Wrap inline code references in single backticks: \`variableName\`
2. EVERY code suggestion MUST use fenced code blocks. A fenced code block has THREE backticks + language on the OPENING line, and THREE backticks ALONE on the CLOSING line. Supported languages: python, javascript, typescript, mermaid, bash, json, yaml, diff.
   CORRECT (within JSON string value): "Explanation text.\\n\\n\`\`\`python\\ndef foo():\\n    pass\\n\`\`\`"
   CORRECT (within JSON string value): "Fix below.\\n\\n\`\`\`javascript\\nconst x = 1;\\n\`\`\`"
   WRONG: "python\\ndef foo():\\n    pass"  ← missing triple-backtick fences entirely
   WRONG: "\`\`\`python\\ndef foo():\\n    pass"  ← missing closing triple-backtick fence
3. For Mermaid architecture diagrams in the summary field, use: \`\`\`mermaid\\ngraph TD\\n    A-->B\\n\`\`\`
4. Use \\n for newlines within the JSON string value
5. Explain the "why" before showing the fix
6. Never output a bare language identifier (like python or javascript) on its own line followed by code — always wrap with triple backticks

If no issues found, return an empty issues array.

### JSON Schema

{
  "analysis_scratchpad": "Step 1: Trace data flow... Step 2: Check trust boundaries... Step 3: Evaluate coupling...",
  "summary": "# PR Summary\\n\\n## Purpose & Scope\\n...\\n\\n## Architecture\\n...\\n\\n\`\`\`mermaid\\ngraph TD\\n...\`\`\`\\n\\n## Code Quality\\n...\\n\\n## Risks & Concerns\\n...\\n\\n## Verdict\\n**Approve** — ...",
  "issues": [
    {
      "file": "path/to/file.py",
      "line": 65,
      "severity": "critical",
      "category": "SECURITY",
      "comment": "Explanation of the issue and attack narrative.\\n\\n\`\`\`python\\n# Suggested fix\\ndef fixed_function():\\n    pass\\n\`\`\`"
    }
  ]
}

---

IMPORTANT: This is a one-shot automated review. Respond with the JSON above and nothing else. Do NOT ask follow-up questions, do NOT wait for further input, and do NOT create any pull requests or branches. End the session immediately after your response.

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

		throw new Error(
			`Jules session timed out after ${SESSION_TIMEOUT_MS / 60000} minutes`,
		);
	}

	async _listActivities(sessionId) {
		const res = await fetch(
			`${JULES_BASE}/${sessionId}/activities?pageSize=30`,
			{
				headers: this.headers,
			},
		);

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
			JSON.stringify(activitiesResponse, null, 2),
		);

		const activities =
			activitiesResponse.activities || activitiesResponse || [];
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
				console.log(
					"[Jules] Found agent text in activity:",
					rawText.substring(0, 200),
				);
			}

			// Also check for nested textContent or parts (Gemini-style)
			if (activity.parts) {
				for (const part of activity.parts) {
					if (part.text) {
						rawText = part.text;
						console.log(
							"[Jules] Found text in activity.parts:",
							rawText.substring(0, 200),
						);
					}
				}
			}
		}

		// Fallback: check session outputs (Jules API can embed results here)
		if (!rawText && completedSession?.outputs) {
			console.log(
				"[Jules] Checking completedSession.outputs:",
				JSON.stringify(completedSession.outputs),
			);
			for (const output of completedSession.outputs) {
				const text =
					output.text || output.content || output.message || output.response;
				if (text) {
					rawText = typeof text === "string" ? text : JSON.stringify(text);
					console.log(
						"[Jules] Found text in session outputs:",
						rawText.substring(0, 200),
					);
				}
			}
		}

		// If still nothing found, try to extract text from the entire response
		if (!rawText && activitiesResponse) {
			const fullStr = JSON.stringify(activitiesResponse);
			// Look for our expected JSON structure anywhere in the response
			const jsonMatch = fullStr.match(
				/\{[^{}]*"summary"\s*:\s*"[^"]*"[^{}]*"issues"\s*:\s*\[[\s\S]*?\]\s*\}/,
			);
			if (jsonMatch) {
				rawText = jsonMatch[0];
				console.log("[Jules] Extracted JSON from raw response via regex");
			}
		}

		if (!rawText) {
			console.error(
				"[Jules] Could not find agent response in activities. Full response:",
				JSON.stringify(activitiesResponse),
			);
			return {
				summary: "### Code Review\n\nNo response received from Jules.",
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
			const jsonMatch = jsonStr.match(
				/\{[\s\S]*"summary"[\s\S]*"issues"[\s\S]*\}/,
			);
			if (jsonMatch) {
				jsonStr = jsonMatch[0];
			}

			const parsed = JSON.parse(jsonStr);
			const comments = [];
			const validFiles = new Set(files.map((f) => f.filename));

			if (parsed.issues && Array.isArray(parsed.issues)) {
				for (const issue of parsed.issues) {
					if (validFiles.has(issue.file) && issue.line && issue.comment) {
						const category =
							issue.category || issue.severity?.toUpperCase() || "NOTE";
						const severity = issue.severity?.toUpperCase() || "NOTE";

						const fixedComment = this._fixCodeBlocks(issue.comment);
						comments.push({
							path: issue.file,
							line: issue.line,
							side: "RIGHT",
							body: `**[${category}]** (${severity}): ${fixedComment}`,
						});
					}
				}
			}

			const rawSummary =
				parsed.summary ||
				(comments.length === 0
					? "No issues found. Code looks good."
					: `Found ${comments.length} issue(s).`);
			const summary = this._fixCodeBlocks(rawSummary);

			return {
				summary: `### Code Review\n\n${summary}`,
				comments,
			};
		} catch {
			// If we can't parse JSON, just post the raw text as a summary
			return {
				summary: `### Code Review\n\n${rawText}`,
				comments: [],
			};
		}
	}

	/**
	 * Repairs malformed code blocks in Jules output.
	 * Handles: bare language tags without fences, unclosed fences.
	 * @param {string} text - The markdown text to fix
	 * @returns {string} - Fixed markdown text
	 */
	_fixCodeBlocks(text) {
		if (!text || typeof text !== "string") return text;

		const LANGS =
			"python|javascript|typescript|mermaid|bash|shell|json|yaml|diff|html|css|sql|go|ruby|java|csharp|c|cpp";

		// Pattern 1: Bare language tag on its own line (not preceded by ```)
		// Matches: "\npython\n<code>" but NOT "```python\n<code>"
		// We look for a line that is ONLY a language name, followed by code lines,
		// up to the next blank line, next bare language tag, or end of string.
		const bareTagRegex = new RegExp(
			`(^|\\n)(?!\`\`\`)(${LANGS})\\n([\\s\\S]*?)(?=\\n\\n|\\n(?:${LANGS})\\n|$)`,
			"gi",
		);

		let result = text.replace(bareTagRegex, (match, prefix, lang, code) => {
			// Don't fix if the code is empty or only whitespace
			if (!code.trim()) return match;
			return `${prefix}\`\`\`${lang}\n${code}\n\`\`\``;
		});

		// Pattern 2: Unclosed code fences — opening ``` with lang but no closing ```
		// Count opening fences (``` followed by a language tag) vs closing fences (standalone ```)
		const openFences = (result.match(/```[a-zA-Z]+/g) || []).length;
		const closeFences = (result.match(/^```\s*$/gm) || []).length;
		if (openFences > closeFences) {
			// Append the missing closing fences
			const missing = openFences - closeFences;
			for (let i = 0; i < missing; i++) {
				result = `${result.trimEnd()}\n\`\`\``;
			}
		}

		return result;
	}

	_sleep(ms) {
		return new Promise((resolve) => setTimeout(resolve, ms));
	}
}
