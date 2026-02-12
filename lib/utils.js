/**
 * Utility functions for parsing diffs and filtering files
 */

import { minimatch } from "minimatch";

/**
 * Parses a raw unified diff string into structured file objects.
 * @param {string} diffStr - The raw diff string
 * @returns {Array<{path: string, content: string}>}
 */
export const parseDiff = (diffStr) => {
	const files = [];
	let currentFile = null;

	const lines = diffStr.split("\n");

	for (const line of lines) {
		if (line.startsWith("diff --git")) {
			if (currentFile) files.push(currentFile);
			currentFile = { path: "", content: "" };
		} else if (line.startsWith("+++ b/")) {
			if (currentFile) currentFile.path = line.substring(6);
		} else if (currentFile) {
			currentFile.content += `${line}\n`;
		}
	}

	if (currentFile) files.push(currentFile);
	return files;
};

/**
 * Filters out files that shouldn't be reviewed.
 * Uses minimatch for glob-based pattern matching.
 * @param {Array} files
 * @param {string[]} ignorePatterns - glob patterns
 * @returns {Array}
 */
export const filterFiles = (files, ignorePatterns = []) => {
	const defaultIgnore = [
		"*.md",
		"*.txt",
		"*.png",
		"*.jpg",
		"*.svg",
		"*.json",
		"*.lock",
		"docs/**",
		"test/**",
		"dist/**",
		"node_modules/**",
	];

	const patterns = [...defaultIgnore, ...ignorePatterns];

	return files.filter((file) => {
		if (!file.path) return false;
		return !patterns.some((pattern) => {
			try {
				return minimatch(file.path, pattern, { matchBase: true });
			} catch {
				return false;
			}
		});
	});
};
