// The connector's window (ui/) talks to the app through this, and nothing else: what it shows
// (state, and every change to it) and what its buttons do (see controller.cjs).
const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('connector', {
  state: () => ipcRenderer.invoke('connector:state'),
  do: (action, args) => ipcRenderer.invoke('connector:do', action, args ?? {}),
  onState: (cb) => ipcRenderer.on('connector:state', (_e, s) => cb(s)),
});
