#!/usr/bin/env node
// `kui` from npm: runs this machine's runner binary (index.js says where it
// comes from) with these arguments and this terminal, and leaves with its
// exit code — or, when a signal ended it, by the same signal.
//
// Signals: the runner is a child in this process's group, so what a
// terminal sends the group (Ctrl-C's SIGINT, a closed terminal's SIGHUP)
// reaches it without help. Forwarding those as well would deliver each
// twice, and the runner reads a second SIGINT as "end at once" (the first
// closes the window and runs `on_exit`), so they are forwarded only when
// no stream is a terminal, where nothing else could have sent the group
// one. SIGTERM, which a supervisor sends this process alone, always is.
// Node itself is kept alive through them until the runner has gone. On
// Windows the console delivers Ctrl-C to every process attached to it, so
// there is nothing to forward, only Node to keep alive.
'use strict';
const { spawn } = require('node:child_process');
const { constants } = require('node:os');

let bin;
try {
  bin = require('../index.js').path;
} catch (err) {
  process.stderr.write(`${err.message}\n`);
  process.exit(1);
}

const child = spawn(bin, process.argv.slice(2), { stdio: 'inherit' });

const terminal = process.stdin.isTTY || process.stdout.isTTY || process.stderr.isTTY;
const signals =
  process.platform === 'win32'
    ? { SIGINT: false, SIGBREAK: false }
    : { SIGINT: !terminal, SIGHUP: !terminal, SIGQUIT: !terminal, SIGTERM: true };
const handlers = {};
for (const [sig, forward] of Object.entries(signals)) {
  handlers[sig] = () => {
    if (forward && child.exitCode === null && child.signalCode === null) child.kill(sig);
  };
  process.on(sig, handlers[sig]);
}

child.on('error', (err) => {
  process.stderr.write(`kui: could not start ${bin}: ${err.message}\n`);
  process.exit(1);
});
child.on('exit', (code, signal) => {
  if (signal) {
    // Leave the way the runner did, so a shell sees the same death.
    for (const [sig, h] of Object.entries(handlers)) process.off(sig, h);
    process.kill(process.pid, signal);
    // A signal this process cannot die of still leaves as a shell would say.
    process.exitCode = 128 + (constants.signals[signal] ?? 0);
    return;
  }
  process.exit(code ?? 1);
});
