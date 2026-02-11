import { strict as assert } from "node:assert";
import { test } from "node:test";
import { parseDiff, filterFiles } from "../lib/utils.js";

test("parseDiff should parse a simple diff", () => {
  const sampleDiff = `diff --git a/file1.js b/file1.js
index 123..456 100644
--- a/file1.js
+++ b/file1.js
@@ -1,3 +1,3 @@
 const a = 1;
-const b = 2;
+const b = 3;
 const c = 4;
diff --git a/README.md b/README.md
index 111..222 100644
--- a/README.md
+++ b/README.md
@@ -1 +1 @@
-# Title
+# New Title
`;

  const result = parseDiff(sampleDiff);
  assert.equal(result.length, 2);
  assert.equal(result[0].path, "file1.js");
  assert.ok(result[0].content.includes("const b = 3;"));
  assert.equal(result[1].path, "README.md");
});

test("filterFiles should exclude ignored files", () => {
  const files = [
    { path: "src/app.js" },
    { path: "docs/guide.md" },
    { path: "README.md" },
    { path: "package-lock.json" },
    { path: "test/foo.test.js" },
  ];

  const filtered = filterFiles(files);
  assert.equal(filtered.length, 1);
  assert.equal(filtered[0].path, "src/app.js");
});

test("filterFiles should support custom ignore patterns", () => {
  const files = [
    { path: "src/app.js" },
    { path: "src/generated/types.ts" },
  ];

  const filtered = filterFiles(files, ["src/generated/**"]);
  assert.equal(filtered.length, 1);
  assert.equal(filtered[0].path, "src/app.js");
});
