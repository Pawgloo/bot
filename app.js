/**
 * Main Probot application
 *
 * Triggers:
 *   - pull_request.opened / synchronize  → automatic review
 *   - issue_comment.created "/pawgloo-review" → manual re-review
 *
 * @param {import('probot').Probot} app
 */
import { JulesReviewer } from "./lib/jules.js";

const MAX_PATCH_LENGTH = process.env.MAX_PATCH_LENGTH
	? +process.env.MAX_PATCH_LENGTH
	: Infinity;

// Configurable ignore patterns (comma-separated in .env)
const IGNORE_PATTERNS = (
	process.env.IGNORE_PATTERNS ||
	"*.txt,*.lock,*.png,*.jpg,*.svg,*.ico,dist/,node_modules/"
)
	.split(",")
	.map((p) => p.trim())
	.filter(Boolean);

export default (app) => {
	app.log.info("🤖 Code Review bot loaded!");
	app.log.info(
		`Registered events: pull_request.opened, pull_request.synchronize, issue_comment.created`,
	);
	app.log.info(`Ignore patterns: ${IGNORE_PATTERNS.join(", ")}`);

	// ─── Debug: log all incoming events ───────────────────────
	app.onAny(async (context) => {
		app.log.info(
			{ event: context.name, action: context.payload.action },
			"Event received",
		);
	});

	// ─── Core review logic ────────────────────────────────────
	const analyzeAndReview = async (context, pr) => {
		const repo = context.repo();
		app.log.info(`Starting review for PR #${pr.number}: ${pr.html_url}`);

		try {
			// 1. Compare commits to get changed files with patches
			const { data } = await context.octokit.repos.compareCommits({
				owner: repo.owner,
				repo: repo.repo,
				base: pr.base.sha,
				head: pr.head.sha,
			});

			let changedFiles = data.files || [];
			app.log.info(`Found ${changedFiles.length} changed file(s) in PR`);

			// 2. Filter files
			changedFiles = changedFiles.filter((file) => {
				// Only review modified or added files
				const allowedStatuses = ["modified", "added", "renamed", "copied"];
				if (!allowedStatuses.includes(file.status)) {
					app.log.info(`Skipping ${file.filename} (status: ${file.status})`);
					return false;
				}

				// Skip files matching ignore patterns
				for (const pattern of IGNORE_PATTERNS) {
					if (pattern.endsWith("/")) {
						if (file.filename.startsWith(pattern)) {
							app.log.info(
								`Skipping ${file.filename} (matches pattern: ${pattern})`,
							);
							return false;
						}
					} else if (pattern.startsWith("*.")) {
						if (file.filename.endsWith(pattern.slice(1))) {
							app.log.info(
								`Skipping ${file.filename} (matches pattern: ${pattern})`,
							);
							return false;
						}
					} else {
						if (file.filename === pattern) {
							app.log.info(
								`Skipping ${file.filename} (matches pattern: ${pattern})`,
							);
							return false;
						}
					}
				}

				// Skip files with patches that are too large
				if (!file.patch || file.patch.length > MAX_PATCH_LENGTH) {
					app.log.info(`Skipping ${file.filename} (patch too large or empty)`);
					return false;
				}

				app.log.info(`✓ Will review: ${file.filename}`);
				return true;
			});

			if (changedFiles.length === 0) {
				app.log.info("No relevant files to review after filtering.");
				return;
			}

			app.log.info(`Reviewing ${changedFiles.length} file(s)...`);

			// 3. Call Jules
			const mode = process.env.JULES_MODE || "SPEED";
			const reviewer = new JulesReviewer(process.env.JULES_API_KEY, mode);
			const filesForReview = changedFiles.map((f) => ({
				filename: f.filename,
				patch: f.patch,
			}));

			// Pass PR metadata for richer prompt context
			const prMeta = {
				title: pr.title || "",
				body: pr.body || "",
				author: pr.user?.login || "unknown",
				baseBranch: pr.base?.ref || "main",
				headBranch: pr.head?.ref || "unknown",
			};

			const review = await reviewer.analyze(filesForReview, prMeta);

			// 4. Post Review via GitHub API
			//    Map each comment's `line` to a valid position in the diff
			const diffLineMap = new Map();
			for (const file of changedFiles) {
				diffLineMap.set(file.filename, parseDiffLines(file.patch));
			}

			const validComments = [];
			const orphanComments = [];

			for (const comment of review.comments) {
				const validLines = diffLineMap.get(comment.path);
				if (validLines && validLines.has(comment.line)) {
					validComments.push({
						path: comment.path,
						line: comment.line,
						side: comment.side || "RIGHT",
						body: comment.body,
					});
				} else if (validLines) {
					// File exists in diff but line doesn't — save as orphan
					orphanComments.push(comment);
				}
			}

			let reviewBody = validComments.length
				? review.summary
				: review.summary || "LGTM 👍";

			if (orphanComments.length > 0) {
				reviewBody +=
					"\n\n---\n**Additional comments** (on lines outside the diff):\n\n";
				for (const c of orphanComments) {
					reviewBody += `- **${c.path}:${c.line}** — ${c.body}\n`;
				}
			}

			try {
				await context.octokit.pulls.createReview({
					owner: repo.owner,
					repo: repo.repo,
					pull_number: pr.number,
					commit_id: pr.head.sha,
					body: reviewBody,
					event: "COMMENT",
					comments: validComments,
				});

				app.log.info(
					`✅ Review posted with ${validComments.length} inline comment(s).`,
				);
			} catch (reviewErr) {
				app.log.error(
					`createReview failed: ${reviewErr.message}, falling back to comment`,
				);
				// Fallback: post as a plain issue comment
				let fallbackBody = reviewBody;
				for (const c of validComments) {
					fallbackBody += `\n- **${c.path}:${c.line}** — ${c.body}`;
				}
				await context.octokit.issues.createComment({
					...context.repo(),
					issue_number: pr.number,
					body: fallbackBody,
				});
			}
		} catch (error) {
			app.log.error(`Review failed: ${error.message}`);
			app.log.error(error.stack);

			// Post error as a comment so the user knows something went wrong
			try {
				await context.octokit.issues.createComment({
					...context.repo(),
					issue_number: pr.number,
					body: `### Code Review\n\n❌ **Error during review**: ${error.message}\n\nPlease check the bot logs or retry with \`/pawgloo-review\`.`,
				});
			} catch (commentError) {
				app.log.error(`Failed to post error comment: ${commentError.message}`);
			}
		}
	};

	// ─── Automatic trigger ────────────────────────────────────
	app.on(
		["pull_request.opened", "pull_request.synchronize"],
		async (context) => {
			const pr = context.payload.pull_request;
			app.log.info(
				`🔔 Auto-trigger fired: ${context.payload.action} on PR #${pr.number} by ${pr.user?.login}`,
			);

			if (pr.state === "closed" || pr.locked) {
				app.log.info("Skipping closed/locked PR");
				return;
			}

			// Skip draft PRs (optional, configurable)
			if (pr.draft && process.env.SKIP_DRAFT_PRS !== "false") {
				app.log.info(`Skipping draft PR #${pr.number}`);
				return;
			}

			await analyzeAndReview(context, pr);
		},
	);

	// ─── Manual trigger via comment ───────────────────────────
	app.on("issue_comment.created", async (context) => {
		const { issue, comment } = context.payload;

		// Must be a PR comment with the magic command
		if (!issue.pull_request) return;

		const trimmed = comment.body.trim().toLowerCase();
		if (trimmed !== "/pawgloo-review" && trimmed !== "/jules review") return;

		app.log.info(
			`Manual trigger by ${comment.user.login} on PR #${issue.number}`,
		);

		// React with 🚀 to acknowledge
		await context.octokit.reactions.createForIssueComment({
			...context.repo(),
			comment_id: comment.id,
			content: "rocket",
		});

		// Fetch full PR object (issue payload is partial)
		const { data: pr } = await context.octokit.pulls.get({
			...context.repo(),
			pull_number: issue.number,
		});

		await analyzeAndReview(context, pr);
	});
};

/**
 * Parse a unified diff patch and return the set of valid
 * new-file (right-side) line numbers that GitHub will accept
 * for inline review comments.
 *
 * @param {string} patch
 * @returns {Set<number>}
 */
function parseDiffLines(patch) {
	const lines = new Set();
	if (!patch) return lines;

	let newLine = 0;
	for (const raw of patch.split("\n")) {
		const hunkHeader = raw.match(/^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
		if (hunkHeader) {
			newLine = parseInt(hunkHeader[1], 10);
			continue;
		}
		if (raw.startsWith("-")) continue; // deleted line — not in new file
		if (raw.startsWith("+") || raw.startsWith(" ")) {
			lines.add(newLine);
			newLine++;
		}
	}
	return lines;
}
