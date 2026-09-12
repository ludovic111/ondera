import { app, BrowserWindow, shell } from 'electron';
import { join } from 'node:path';
import { color, size } from '../src/theme/tokens';

function createWindow(): void {
  const win = new BrowserWindow({
    width: size.windowWidth,
    height: size.windowHeight,
    minWidth: size.minWindowWidth,
    minHeight: size.minWindowHeight,
    show: false,
    backgroundColor: color.timeline,
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'default',
    trafficLightPosition: { x: 12, y: 8 },
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  win.on('ready-to-show', () => win.show());
  // Pinch and ctrl+wheel are timeline zoom, never page zoom.
  void win.webContents.setVisualZoomLevelLimits(1, 1);

  win.webContents.setWindowOpenHandler(({ url }) => {
    void shell.openExternal(url);
    return { action: 'deny' };
  });

  const devUrl = process.env['ELECTRON_RENDERER_URL'];
  if (devUrl) void win.loadURL(devUrl);
  else void win.loadFile(join(__dirname, '../renderer/index.html'));
}

void app.whenReady().then(() => {
  createWindow();
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});
