export interface AudioExportReport {
  path?: string;
  directory?: string;
  seconds?: number;
  clippedSamples?: number;
  /** Ogg Vorbis only: the average bitrate the chosen quality came to. */
  kbps?: number;
  warnings?: string[];
  files?: AudioExportReport[];
}

export function exportProblem({
  range,
  start,
  end,
  tail,
  stems,
  trackCount,
}: {
  range: boolean;
  start: number;
  end: number;
  tail: number;
  stems: boolean;
  trackCount: number;
}): string {
  if (!Number.isFinite(tail) || tail < 0 || tail > 120)
    return "Choose a release tail between 0 and 120 seconds.";
  if (
    range &&
    (!Number.isFinite(start) ||
      !Number.isFinite(end) ||
      start < 1 ||
      end <= start)
  )
    return "The end bar must be after the start bar (bar 1 or later).";
  if (stems && trackCount === 0)
    return "Select at least one track to export as a stem.";
  return "";
}

export function exportSummary(report: AudioExportReport): string {
  const lines = ["Export complete."];
  if (report.directory) lines.push(report.directory);
  const files = report.files ?? [report];
  if (report.files) lines.push(`${report.files.length} stem files`);
  for (const file of files) {
    if (file.path) lines.push(file.path);
    if (file.seconds != null)
      lines.push(`Duration: ${file.seconds.toFixed(2)} seconds`);
    if (file.kbps != null)
      lines.push(`Ogg Vorbis, about ${Math.round(file.kbps)} kbit/s`);
    if (file.clippedSamples)
      lines.push(
        file.path?.toLowerCase().endsWith(".ogg")
          ? `${file.clippedSamples} samples are above full scale and will clip when played. Lower the mix level.`
          : `${file.clippedSamples} samples clipped. Lower the mix level or export 32-bit float to preserve headroom.`,
      );
    lines.push(...(file.warnings ?? []));
  }
  if (report.files) lines.push(...(report.warnings ?? []));
  return lines.join("\n");
}
