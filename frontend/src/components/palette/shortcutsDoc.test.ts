import { readFileSync, writeFileSync } from "node:fs";
import { expect, it } from "vitest";
import { shortcutGroups } from "./Shortcuts";

// docs/SHORTCUTS.md is generated from the action table, like the in-app sheet, so the
// documentation cannot drift from the keyboard handler. Regenerate with
// `ONDERA_BLESS=1 npm --prefix frontend test -- shortcutsDoc`.
function reference(): string {
  const lines = [
    "# Keyboard shortcuts",
    "",
    "<!-- Generated from frontend/src/state/actions.ts by frontend/src/components/palette/shortcutsDoc.test.ts. Do not edit by hand: run `ONDERA_BLESS=1 npm --prefix frontend test -- shortcutsDoc`. -->",
    "",
    "The same list is in the app: Help > Shortcuts and Help (⌘/). ⌘ is Ctrl on Windows and Linux, ⌥ is Alt.",
    "",
  ];
  for (const group of shortcutGroups()) {
    lines.push(`## ${group.title}`, "", "| Action | Keys |", "|---|---|");
    for (const row of group.rows) lines.push(`| ${row.label} | \`${row.keys}\` |`);
    lines.push("");
  }
  lines.push(
    "## Musical typing",
    "",
    "With musical typing on (⌘K), the letter row from A to ; plays notes on the selected instrument track, and Z / X shift the octave.",
    "",
  );
  return lines.join("\n");
}

it("docs/SHORTCUTS.md matches the action table", () => {
  const url = new URL("../../../../docs/SHORTCUTS.md", import.meta.url);
  const fresh = reference();
  if (process.env.ONDERA_BLESS) {
    writeFileSync(url, fresh);
    return;
  }
  let current = "";
  try {
    current = readFileSync(url, "utf8").replace(/\r\n/g, "\n");
  } catch {
    // Missing: fall through to the failure below.
  }
  expect(
    current === fresh,
    "docs/SHORTCUTS.md is stale: run ONDERA_BLESS=1 npm --prefix frontend test -- shortcutsDoc",
  ).toBe(true);
});
