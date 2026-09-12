import { app, BrowserWindow, dialog, ipcMain, Menu, shell } from 'electron';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { basename, dirname, extname, join } from 'node:path';
import { color, size } from '../src/theme/tokens';

function createWindow(): BrowserWindow {
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
  return win;
}

/**
 * Native menu: only what the renderer cannot do itself (quit, text editing
 * roles for the agent input, dev tools). Application commands live in the
 * renderer's own menus so the same table drives keyboard, menu and agent.
 */
function installMenu(): void {
  const template: Electron.MenuItemConstructorOptions[] = [
    ...(process.platform === 'darwin' ? [{ role: 'appMenu' as const }] : []),
    { label: 'Text', submenu: [{ role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'selectAll' }] },
    { label: 'Window', submenu: [{ role: 'minimize' }, { role: 'zoom' }, { role: 'togglefullscreen' }, { type: 'separator' }, { role: 'toggleDevTools' }, { role: 'reload' }] },
  ];
  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

const SESSION_FILTER = { name: 'Ondera Session', extensions: ['ondera'] };
const AUDIO_FILTER = { name: 'Audio', extensions: ['wav', 'aif', 'aiff', 'mp3', 'flac', 'ogg', 'm4a', 'webm'] };

function installIpc(): void {
  ipcMain.handle('session:open', async (e) => {
    const win = BrowserWindow.fromWebContents(e.sender);
    const result = await dialog.showOpenDialog(win!, { properties: ['openFile'], filters: [SESSION_FILTER] });
    const path = result.filePaths[0];
    if (result.canceled || !path) return null;
    return { path, json: await readFile(path, 'utf8') };
  });

  ipcMain.handle('session:save', async (e, path: string | null, defaultName: string, json: string) => {
    const win = BrowserWindow.fromWebContents(e.sender);
    let target = path;
    if (!target) {
      const result = await dialog.showSaveDialog(win!, {
        defaultPath: defaultName.endsWith('.ondera') ? defaultName : `${defaultName}.ondera`,
        filters: [SESSION_FILTER],
      });
      if (result.canceled || !result.filePath) return null;
      target = extname(result.filePath) ? result.filePath : `${result.filePath}.ondera`;
    }
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, json, 'utf8');
    app.addRecentDocument(target);
    return target;
  });

  ipcMain.handle('audio:import', async (e) => {
    const win = BrowserWindow.fromWebContents(e.sender);
    const result = await dialog.showOpenDialog(win!, { properties: ['openFile', 'multiSelections'], filters: [AUDIO_FILTER] });
    if (result.canceled) return [];
    return Promise.all(
      result.filePaths.map(async (p) => {
        const buf = await readFile(p);
        return { name: basename(p), data: buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength) };
      }),
    );
  });

  ipcMain.handle('file:save', async (e, defaultName: string, data: ArrayBuffer, filterName: string, extensions: string[]) => {
    const win = BrowserWindow.fromWebContents(e.sender);
    const result = await dialog.showSaveDialog(win!, { defaultPath: defaultName, filters: [{ name: filterName, extensions }] });
    if (result.canceled || !result.filePath) return false;
    await writeFile(result.filePath, Buffer.from(data));
    return true;
  });
}

void app.whenReady().then(() => {
  installMenu();
  installIpc();
  createWindow();
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});
