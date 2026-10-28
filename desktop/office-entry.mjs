// Started by the Windows app (desktop/main.cjs) with the node.exe it ships: runs the office the way
// `agent-office` does, but hands the app its sign-in links over the IPC channel instead of printing
// them in a terminal, and closes when the app tells it to (or goes away).
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// Packaged, this file sits next to the office's dist/ (resources/office); run from a checkout, the
// app points it at the repository root.
const root = process.env.AGENT_OFFICE_ROOT || path.dirname(fileURLToPath(import.meta.url));
const load = (file) => import(pathToFileURL(path.join(root, 'dist', 'server', 'server', file)).href);
const { loadConfig, ensureSelfSigned } = await load('config.js');
const { startServer } = await load('server.js');

const send = (msg) => process.send?.(msg);

const cfg = loadConfig(process.argv.slice(2));
await ensureSelfSigned(cfg);

let office;
try {
  office = await startServer(cfg);
} catch (err) {
  send({ t: 'error', code: err.code, message: err.message });
  process.exit(1);
}

const base = `${cfg.tls ? 'https' : 'http'}://127.0.0.1:${cfg.port}`;
/** A link that signs the app's window (or a browser) in once; an office with accounts only shows its login page. */
const signIn = () => (office.accounts.sharedPassword ? base + office.signInLink() : `${base}/`);

let closing = false;
const close = (keep) => {
  if (closing) return;
  closing = true;
  office.shutdown(keep);
  setTimeout(() => process.exit(0), 300);
};

process.on('message', (msg) => {
  if (msg?.t === 'signIn') send({ t: 'signIn', id: msg.id, url: signIn() });
  else if (msg?.t === 'quit') close(!!msg.keep);
});
// The app crashed or was killed: don't leave an office nobody can see running.
process.on('disconnect', () => close(false));
// Last line of defense, as in cli.ts: one bad request must never take down every running worker.
process.on('unhandledRejection', (err) => console.error('agent-office: unhandled rejection', err));

console.log(`agent-office is open at ${base} (data in ${cfg.dataDir})`);
send({ t: 'ready', url: base, signIn: signIn(), dataDir: cfg.dataDir, projectsDir: office.projectsDir() });
