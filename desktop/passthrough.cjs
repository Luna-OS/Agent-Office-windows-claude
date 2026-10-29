// The office in the normal browser, voice and screen sharing included. Browsers only let a page use
// the microphone and the screen on https:// and on this computer's own addresses, and a server over
// plain http on another machine (an Umbrel at http://100.78.107.162:4600) is neither. So the
// connector passes it on at http://127.0.0.1:<port>: every request and WebSocket goes to the server
// as it came, Host and all, and every answer comes back as it is. The office sees the address the
// browser does, so its sign-in cookie and its same-origin checks hold.
const http = require('node:http');
const net = require('node:net');

/** The first port tried, so the browser finds the office at the same address every time. */
const PORT = 48600;

/** Listens on 127.0.0.1 from `port`, or the next free one: resolves to the server and its port. */
function listen(server, port, tries = 20) {
  return new Promise((resolve, reject) => {
    const attempt = (p) => {
      server.once('error', (err) => (err.code === 'EADDRINUSE' && p < port + tries ? attempt(p + 1) : reject(err)));
      server.listen(p, '127.0.0.1', () => resolve(p));
    };
    attempt(port);
  });
}

/**
 * Passes the server at `target` (an http:// origin) on at 127.0.0.1. Resolves to { origin, close,
 * retarget(origin) }: retarget points it at another server without the browser's address changing.
 */
async function startPassthrough(target, port = PORT) {
  let to = new URL(target);
  const server = http.createServer((req, res) => {
    const up = http.request({ host: to.hostname, port: to.port || 80, method: req.method, path: req.url, headers: req.headers }, (ur) => {
      res.writeHead(ur.statusCode ?? 502, ur.statusMessage, ur.rawHeaders);
      ur.pipe(res);
    });
    up.on('error', (err) => {
      if (res.headersSent) return res.destroy();
      res.writeHead(502, { 'content-type': 'text/html; charset=utf-8' });
      res.end(`<!doctype html><meta charset="utf-8"><title>Agent Office</title><body style="font:16px system-ui;background:#1b1d2a;color:#e8e8f0;padding:40px">` + `<h1>Der Server antwortet nicht</h1><p>${to.origin} ist gerade nicht erreichbar (${err.code ?? err.message}). Läuft die Umbrel-App „Agent Office Vermittler“, und ist dieser Computer im selben Netz?</p><p><a style="color:#f0a487" href="/">Noch einmal versuchen</a></p>`);
    });
    res.on('close', () => up.destroy());
    req.pipe(up);
  });
  // WebSockets: the handshake goes on as it came, then the two sockets are spliced together.
  const spliced = new Set();
  server.on('upgrade', (req, socket, head) => {
    const up = net.connect(Number(to.port || 80), to.hostname);
    spliced.add(socket);
    const lines = [`${req.method} ${req.url} HTTP/${req.httpVersion}`];
    for (let i = 0; i < req.rawHeaders.length; i += 2) lines.push(`${req.rawHeaders[i]}: ${req.rawHeaders[i + 1]}`);
    up.on('connect', () => {
      up.write(lines.join('\r\n') + '\r\n\r\n');
      if (head?.length) up.write(head);
      up.pipe(socket);
      socket.pipe(up);
    });
    const end = () => {
      spliced.delete(socket);
      up.destroy();
      socket.destroy();
    };
    up.on('error', end);
    socket.on('error', end);
    up.on('close', end);
    socket.on('close', end);
  });
  const got = await listen(server, port);
  return {
    origin: `http://127.0.0.1:${got}`,
    retarget: (origin) => void (to = new URL(origin)),
    close: () =>
      new Promise((resolve) => {
        server.close(() => resolve());
        server.closeAllConnections();
        for (const socket of spliced) socket.destroy();
      }),
  };
}

module.exports = { PORT, startPassthrough };
