const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('crystalline', {
  getCatalog: () => ipcRenderer.invoke('cr:getCatalog'),
  rpc: (method, params) => ipcRenderer.invoke('cr:rpc', method, params),
  openBackupsFolder: () => ipcRenderer.invoke('cr:openBackupsFolder'),
  relaunchAdmin: () => ipcRenderer.invoke('cr:relaunchAdmin'),
  windowControl: (action) => ipcRenderer.invoke('cr:windowControl', action),
  onMaximizedChange: (cb) => {
    const handler = (_e, isMax) => cb(isMax);
    ipcRenderer.on('cr:maximized-state', handler);
    return () => ipcRenderer.removeListener('cr:maximized-state', handler);
  },
});
