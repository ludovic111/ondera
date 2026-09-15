export const slashCommands = [
  { name: "rhythm", label: "Open the Euclidean Rhythm Lab", prompt: "" },
  {
    name: "diagnose",
    label: "Diagnose any problem",
    prompt:
      "Diagnose this problem in my project: [describe the problem]. Inspect the relevant audio, MIDI, plugins, routing, files or settings. Identify the cause before making changes, explain it plainly, and verify the fix.",
  },
  {
    name: "mix",
    label: "Review the mix",
    prompt:
      "Inspect this mix and suggest the three most useful improvements. Check gain, panning, effects, routing and audibility. Explain before changing anything.",
  },
  {
    name: "arrange",
    label: "Develop the arrangement",
    prompt:
      "Help develop this song into an arrangement with distinct sections. Inspect the existing music, propose a structure, and preserve the original material.",
  },
  {
    name: "beat",
    label: "Write a drum groove",
    prompt:
      "Add a four-bar drum groove on a new track that fits this project. Check that the new track is audible and explain any Solo or Mute blocker.",
  },
  {
    name: "chords",
    label: "Write a chord progression",
    prompt:
      "Write a four-bar chord progression matching this project, on a new instrument track.",
  },
  {
    name: "humanize",
    label: "Humanize the selected region",
    prompt:
      "Apply subtle timing and velocity humanization to the selected MIDI region. Preserve the pitches and keep notes within its boundaries.",
  },
  {
    name: "quantize",
    label: "Tighten the selected region",
    prompt:
      "Quantize the selected MIDI region to the current grid, preserving its pitches and velocities.",
  },
  {
    name: "scale",
    label: "Fit notes to the song’s key",
    prompt:
      "Fit the selected MIDI region to the project key using the closest notes. Preserve timing and rhythm.",
  },
  {
    name: "velocity",
    label: "Shape the dynamics",
    prompt:
      "Shape the selected MIDI region’s velocities into a musical dynamic phrase. Preserve timing and pitch.",
  },
  {
    name: "repeat",
    label: "Repeat the selected region",
    prompt:
      "Repeat the selected region three times immediately after it, preserving its notes and sound.",
  },
  {
    name: "reverse",
    label: "Reverse the MIDI phrase",
    prompt:
      "Reverse the note timing of the selected MIDI region inside its current boundaries, preserving pitch and velocity.",
  },
  {
    name: "legato",
    label: "Connect a melodic phrase",
    prompt:
      "Make the selected MIDI melody legato by extending each note to the next distinct onset, staying inside the region.",
  },
  {
    name: "plugins",
    label: "Find an instrument or effect",
    prompt:
      "Find suitable installed instruments or effects for this project, including external plugins. Inspect their available parameters and recommend a choice before loading it.",
  },
  {
    name: "explain",
    label: "Understand the project",
    prompt:
      "Explain this project in plain musical language: tracks, arrangement, instruments, effects and routing. Do not change anything.",
  },
  { name: "variation", label: "Create a protected A/B variation", prompt: "" },
  { name: "takes", label: "Compare creative takes", prompt: "" },
  {
    name: "export",
    label: "Prepare an export",
    prompt:
      "Inspect the song and help prepare an audio export. Check its range, tails, levels and output format; ask me for any missing destination or format choice before exporting.",
  },
];
