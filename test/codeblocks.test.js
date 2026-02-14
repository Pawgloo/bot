import { strict as assert } from "node:assert";
import { describe, test } from "node:test";
import { JulesReviewer } from "../lib/jules.js";

// We need a dummy API key to instantiate JulesReviewer
// (only _fixCodeBlocks is tested — no network calls)
const reviewer = new JulesReviewer("test-key-for-unit-tests");

describe("_fixCodeBlocks", () => {
	test("wraps bare python tag with fenced code block", () => {
		const input = "Explanation.\n\npython\ndef foo():\n    pass";
		const result = reviewer._fixCodeBlocks(input);
		assert.ok(
			result.includes("```python\ndef foo():\n    pass\n```"),
			`Expected fenced python block, got:\n${result}`,
		);
	});

	test("wraps bare javascript tag with fenced code block", () => {
		const input = "Text.\n\njavascript\nconst x = 1;\nconsole.log(x);";
		const result = reviewer._fixCodeBlocks(input);
		assert.ok(
			result.includes("```javascript\nconst x = 1;\nconsole.log(x);\n```"),
			`Expected fenced javascript block, got:\n${result}`,
		);
	});

	test("wraps bare mermaid tag with fenced code block", () => {
		const input = "See diagram:\n\nmermaid\ngraph TD\n    A-->B";
		const result = reviewer._fixCodeBlocks(input);
		assert.ok(
			result.includes("```mermaid\ngraph TD\n    A-->B\n```"),
			`Expected fenced mermaid block, got:\n${result}`,
		);
	});

	test("does not modify already valid fenced code blocks", () => {
		const input =
			"Explanation.\n\n```python\ndef foo():\n    pass\n```\n\nMore text.";
		const result = reviewer._fixCodeBlocks(input);
		assert.equal(result, input, "Valid fenced blocks should not be modified");
	});

	test("fixes only broken blocks, leaves valid ones untouched", () => {
		const input =
			"Valid block:\n\n```python\ndef valid():\n    pass\n```\n\nBroken block:\n\njavascript\nconst broken = true;";
		const result = reviewer._fixCodeBlocks(input);
		// Valid one should still be there
		assert.ok(
			result.includes("```python\ndef valid():\n    pass\n```"),
			"Valid block should remain unchanged",
		);
		// Broken one should be fixed
		assert.ok(
			result.includes("```javascript\nconst broken = true;\n```"),
			`Broken block should be wrapped, got:\n${result}`,
		);
	});

	test("auto-closes unclosed fenced code block", () => {
		const input = "Fix:\n\n```python\ndef foo():\n    pass";
		const result = reviewer._fixCodeBlocks(input);
		// Count backtick fences — should have both opening and closing
		const fences = result.match(/```/g) || [];
		assert.ok(
			fences.length >= 2,
			`Expected at least 2 fences (open+close), got ${fences.length}:\n${result}`,
		);
		assert.ok(
			result.trimEnd().endsWith("```"),
			`Should end with closing fence, got:\n${result}`,
		);
	});

	test("handles null/undefined/non-string gracefully", () => {
		assert.equal(reviewer._fixCodeBlocks(null), null);
		assert.equal(reviewer._fixCodeBlocks(undefined), undefined);
		assert.equal(reviewer._fixCodeBlocks(""), "");
	});

	test("wraps bare typescript tag", () => {
		const input =
			"Missing interface.\n\ntypescript\ninterface User {\n  name: string;\n}";
		const result = reviewer._fixCodeBlocks(input);
		assert.ok(
			result.includes(
				"```typescript\ninterface User {\n  name: string;\n}\n```",
			),
			`Expected fenced typescript block, got:\n${result}`,
		);
	});

	test("wraps bare bash tag", () => {
		const input = "Run this:\n\nbash\nnpm install\nnpm test";
		const result = reviewer._fixCodeBlocks(input);
		assert.ok(
			result.includes("```bash\nnpm install\nnpm test\n```"),
			`Expected fenced bash block, got:\n${result}`,
		);
	});

	test("does NOT wrap 'go' as it is a common English word", () => {
		const input = "Ready to\ngo\nnow and do things.";
		const result = reviewer._fixCodeBlocks(input);
		assert.ok(
			!result.includes("```go"),
			`'go' should NOT be treated as a language tag, got:\n${result}`,
		);
		assert.ok(
			result.includes("go\n"),
			`Original 'go' text should be preserved`,
		);
	});

	test("does NOT wrap 'c' as it is a common English letter", () => {
		const input = "Option\nc\nis the best choice.";
		const result = reviewer._fixCodeBlocks(input);
		assert.ok(
			!result.includes("```c"),
			`'c' should NOT be treated as a language tag, got:\n${result}`,
		);
	});

	test("auto-closes unclosed generic fence (no language tag)", () => {
		const input = "See this:\n\n```\nsome code here\nmore code";
		const result = reviewer._fixCodeBlocks(input);
		const fences = result.match(/^```/gm) || [];
		assert.equal(
			fences.length % 2,
			0,
			`Fences should be balanced, got ${fences.length}:\n${result}`,
		);
		assert.ok(
			result.trimEnd().endsWith("```"),
			`Should end with closing fence, got:\n${result}`,
		);
	});
});
