// Runs before first paint (classic script in <head>) so the page never flashes
// the wrong theme. The choice is remembered; the mode follows the system until
// the visitor picks one.
(function () {
  var root = document.documentElement;
  var themes = ['modern', 'skeuo', 'aero'];
  var theme = 'skeuo';
  var mode = null;
  try {
    var t = localStorage.getItem('ondera-theme');
    var m = localStorage.getItem('ondera-mode');
    if (themes.indexOf(t) >= 0) theme = t;
    if (m === 'dark' || m === 'light') mode = m;
  } catch (e) {}
  if (!mode) mode = matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
  root.dataset.theme = theme;
  root.dataset.mode = mode;
})();
