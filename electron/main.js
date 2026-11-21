const { app, BrowserWindow, ipcMain, shell } = require('electron');
const path = require('path');
const fs = require('fs');
const { spawn } = require('child_process');

const APP_ROOT = path.resolve(__dirname, '..');
const CATALOG_PATH = path.join(APP_ROOT, 'tweaks', 'catalog.json');
// Per-user state lives in %APPDATA% — never inside the app folder.
const BACKUPS_DIR = path.join(
  process.env.APPDATA || APP_ROOT,
  'Crystalline',
  'backups'
);
const ENGINE_EXE = path.join(
  APP_ROOT,
  'native-engine',
  'crystalline-engine.exe'
);

let mainWindow = null;
let engineProc = null;
let rpcSeq = 1;
const pendingRpc = new Map();
let stdoutBuffer = '';
let cachedCatalog = null;

function startNativeEngine() {
  if (engineProc && !engineProc.killed) return;
  if (!fs.existsSync(ENGINE_EXE)) {
    console.error('Native Rust engine binary missing at:', ENGINE_EXE);
    return;
  }

  engineProc = spawn(ENGINE_EXE, [], {
    cwd: APP_ROOT,
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true,
  });

  engineProc.stdout.setEncoding('utf8');
  engineProc.stdout.on('data', (chunk) => {
    stdoutBuffer += chunk;
    let nlIdx;
    while ((nlIdx = stdoutBuffer.indexOf('\n')) !== -1) {
      const line = stdoutBuffer.slice(0, nlIdx).trim();
      stdoutBuffer = stdoutBuffer.slice(nlIdx + 1);
      if (!line) continue;
      try {
        const msg = JSON.parse(line);
        const pending = pendingRpc.get(msg.id);
        if (pending) {
          pendingRpc.delete(msg.id);
          if (msg.ok) {
            pending.resolve(msg.result);
          } else {
            pending.reject(new Error(msg.error || 'Native engine RPC error'));
          }
        }
      } catch (err) {
        console.error('Failed parsing engine JSON:', err);
      }
    }
  });

  engineProc.stderr.setEncoding('utf8');
  engineProc.stderr.on('data', (d) => {
    console.error('[crystalline-engine]', d.trim());
  });

  engineProc.on('exit', (code) => {
    engineProc = null;
    for (const [, p] of pendingRpc) {
      p.reject(new Error(`Native engine exited (${code})`));
    }
    pendingRpc.clear();
  });
}

function callEngineRpc(method, params = {}) {
  startNativeEngine();
  return new Promise((resolve, reject) => {
    if (!engineProc || !engineProc.stdin.writable) {
      return reject(new Error('Native Rust engine is not running. Run npm run build first.'));
    }
    const id = rpcSeq++;
    pendingRpc.set(id, { resolve, reject });
    const payload = JSON.stringify({ id, method, params }) + '\n';
    engineProc.stdin.write(payload, 'utf8', (err) => {
      if (err) {
        pendingRpc.delete(id);
        reject(err);
      }
    });
  });
}

function createMainWindow() {
  mainWindow = new BrowserWindow({
    width: 1480,
    height: 920,
    minWidth: 1120,
    minHeight: 700,
    frame: false,
    backgroundColor: '#0d100e',
    title: 'Crystalline — Windows tuning with snapshot safety',
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: true,
    },
  });

  mainWindow.loadFile(path.join(APP_ROOT, 'renderer', 'index.html'));
  mainWindow.once('ready-to-show', () => {
    mainWindow.show();
  });

  mainWindow.on('maximize', () => {
    mainWindow.webContents.send('cr:maximized-state', true);
  });
  mainWindow.on('unmaximize', () => {
    mainWindow.webContents.send('cr:maximized-state', false);
  });
}

ipcMain.handle('cr:getCatalog', async () => {
  if (!cachedCatalog) {
    const raw = await fs.promises.readFile(CATALOG_PATH, 'utf8');
    cachedCatalog = JSON.parse(raw);
  }
  return cachedCatalog;
});

ipcMain.handle('cr:rpc', async (_event, method, params) => {
  return await callEngineRpc(method, params || {});
});

ipcMain.handle('cr:openBackupsFolder', async () => {
  await fs.promises.mkdir(BACKUPS_DIR, { recursive: true });
  return await shell.openPath(BACKUPS_DIR);
});

ipcMain.handle('cr:relaunchAdmin', async () => {
  const exe = process.execPath;
  const appArg = APP_ROOT;
  const psCmd = `Start-Process -FilePath '${exe.replace(/'/g, "''")}' -ArgumentList '"${appArg.replace(/"/g, '`"')}"' -Verb RunAs`;
  spawn('powershell.exe', ['-NoProfile', '-WindowStyle', 'Hidden', '-Command', psCmd], {
    detached: true,
    stdio: 'ignore',
    windowsHide: true,
  }).unref();
  app.quit();
  return true;
});

ipcMain.handle('cr:windowControl', (_event, action) => {
  if (!mainWindow) return false;
  if (action === 'minimize') mainWindow.minimize();
  else if (action === 'maximize') {
    if (mainWindow.isMaximized()) mainWindow.unmaximize();
    else mainWindow.maximize();
  } else if (action === 'close') mainWindow.close();
  return mainWindow.isMaximized();
});

app.whenReady().then(() => {
  startNativeEngine();
  createMainWindow();
});

app.on('window-all-closed', () => {
  if (engineProc && !engineProc.killed) {
    engineProc.kill();
  }
  app.quit();
});
