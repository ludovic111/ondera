/** Aero material recipes. Opaque reading surfaces, translucent window chrome. */
export const aeroVariables: Record<string, string> = {
  "--font-ui": "'Segoe UI', 'Trebuchet MS', 'Manrope', sans-serif",
  "--radius-button": "5px",
  "--radius-control": "9px",
  "--radius-glass": "10px",
  "--gradient-raised":
    "radial-gradient(ellipse at 50% 110%, #ffffffb0, transparent 62%), linear-gradient(#ffffff 0%, #d7e6eb 46%, #9bafba 49%, #c6dce5 100%)",
  "--gradient-transport": "linear-gradient(#f3fafd, #c0dce9)",
  "--gradient-lit":
    "radial-gradient(ellipse at 50% 120%, #c4ff70 0%, transparent 60%), linear-gradient(#e1f8bb 0%, #8ac253 44%, #3a7f1d 48%, #75b533 100%)",
  "--gradient-send-key":
    "radial-gradient(ellipse at 50% 120%, #c4ff70 0%, transparent 60%), linear-gradient(#e1f8bb 0%, #8ac253 44%, #3a7f1d 48%, #75b533 100%)",
  "--shadow-raised":
    "inset 0 1px 0 #ffffffed, inset 0 -2px 3px #32526066, inset 1px 0 1px #ffffffb0, inset -1px 0 1px #ffffff70, 0 0 0 1px #486a7a, 0 2px 3px #132b4266",
  "--shadow-raised-sm":
    "inset 0 1px 0 #fff, inset 0 0 0 1px #a0bac8, 0 1px 1px #31546c1f",
  "--shadow-pressed":
    "inset 0 1px 3px #31546c40, inset 0 0 0 1px #699eb9, 0 1px 0 #fff",
  "--shadow-lit":
    "inset 0 1px 1px #fbffe6, inset 0 -2px 4px #1c521a99, 0 0 0 1px #315d25, 0 2px 4px #112c2460",
  "--color-accent-ink": "#102908",
  "--shadow-segment":
    "inset 0 1px 0 #fff, inset 0 0 0 1px #7ba8bf, 0 1px 2px #31546c22",
  "--shadow-groove":
    "inset 0 1px 3px #31546c33, inset 0 0 0 1px #a6bfcc, 0 1px 0 #fff",
  "--shadow-well-input": "inset 0 1px 3px #31546c20, inset 0 0 0 1px #a6bfcc",
  "--shadow-well-deep":
    "inset 0 1px 4px #23445240, inset 0 0 0 1px #829fae, 0 1px 0 #fff",
  "--shadow-panel-agent": "inset 1px 0 0 #a6c4d5, -3px 0 12px #30566b0a",
  "--shadow-panel-left": "inset -1px 0 0 #b1cbd8",
  "--shadow-panel-right": "inset 1px 0 0 #b1cbd8",
  "--shadow-editor-pane": "0 -2px 6px #30566b16, inset 0 1px 0 #fff",
  "--shadow-transport": "inset 0 1px 0 #ffffffb3, 0 1px 3px #30566b40",
  "--shadow-title-bar": "inset 0 1px 0 #ffffffb3, inset 0 -1px 0 #83b0c6",
  "--shadow-header-cell":
    "inset -1px 0 0 #b1cbd8, inset 0 -1px 0 #bed2dd, inset 0 1px 0 #fff",
  "--shadow-divider": "inset 0 -1px 0 #bfd4df",
  "--shadow-menu":
    "inset 0 1px 0 #fff, 0 0 0 1px #91b2c5, 0 8px 24px #23475d40",
};
