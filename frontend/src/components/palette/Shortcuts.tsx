import { actions } from "../../state/actions";
import { formatShortcut } from "../../state/shortcuts";
import styles from "./Shortcuts.module.css";

const FIXED: [string, string][] = [
  ["New session", "⌘N"],
  ["Open…", "⌘O"],
  ["Save", "⌘S"],
  ["Save as…", "⇧⌘S"],
  ["Import audio…", "⌘I"],
  ["Export audio…", "⌘B"],
  ["Settings…", "⌘,"],
  ["Musical typing", "⌘K"],
];

const GROUPS: [string, (id: string) => boolean][] = [
  [
    "Transport",
    (id) =>
      /^(togglePlay|stop|record|cycle|returnToStart|rewind|forward|metronome)$/.test(
        id,
      ),
  ],
  [
    "Editing",
    (id) =>
      /^(undo|redo|copy|cut|paste|deleteSelection|duplicateClip|splitAtPlayhead|openInEditor|transpose.*)$/.test(
        id,
      ),
  ],
  ["Tracks", (id) => /Track$/.test(id)],
  ["View and tools", () => true],
];

/**
 * The cheat sheet. It is generated from the action table, so it cannot drift
 * from what the keyboard handler and the menus actually do.
 */
export function ShortcutList() {
  const bound = Object.values(actions).filter((a) => a.shortcut);
  const taken = new Set<string>();
  const groups = GROUPS.map(([title, belongs]) => {
    const rows = bound.filter((a) => !taken.has(a.id) && belongs(a.id));
    rows.forEach((a) => taken.add(a.id));
    return { title, rows };
  });
  return (
    <div className={styles.sheet}>
      {groups.map(({ title, rows }) => (
        <section key={title}>
          <h3>{title}</h3>
          <dl>
            {rows.map((a) => (
              <div key={a.id}>
                <dt>{a.label}</dt>
                <dd>{formatShortcut(a.shortcut!)}</dd>
              </div>
            ))}
          </dl>
        </section>
      ))}
      <section>
        <h3>Session</h3>
        <dl>
          {FIXED.map(([label, key]) => (
            <div key={label}>
              <dt>{label}</dt>
              <dd>{key}</dd>
            </div>
          ))}
        </dl>
      </section>
      <p className={styles.note}>
        ⌘ is Ctrl on Windows and Linux. With musical typing on, A–; play notes
        and Z / X shift the octave. In the arrangement: double-click a lane to
        create a region, drag edges to resize, drag the ruler to set the cycle,
        drop audio files from your file manager to import them.
      </p>
    </div>
  );
}
