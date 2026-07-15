#!/usr/bin/env node

import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';

const DEFAULT_TIMEOUT_MS = Number(process.env.CAIPI_CLI_SMOKE_TIMEOUT_MS ?? 180_000);
const backendArg = process.argv.find((arg) => arg.startsWith('--backend='));
const selectedBackend = backendArg?.split('=')[1] ?? process.env.CAIPI_CLI_SMOKE_BACKEND ?? 'both';

const VALID_BACKENDS = new Set(['both', 'claude', 'codex']);
if (!VALID_BACKENDS.has(selectedBackend)) {
  throw new Error(`Invalid backend "${selectedBackend}". Use one of: both, claude, codex.`);
}

function log(message) {
  console.log(`[cli-smoke] ${message}`);
}

function kill(child) {
  if (!child.killed) {
    child.kill('SIGTERM');
    setTimeout(() => {
      if (!child.killed) child.kill('SIGKILL');
    }, 1_000).unref();
  }
}

function withTimeout(label, timeoutMs, onTimeout) {
  return setTimeout(() => {
    onTimeout(new Error(`${label} timed out after ${timeoutMs}ms`));
  }, timeoutMs);
}

async function makeTempProject(prefix) {
  const dir = await mkdtemp(join(tmpdir(), `${prefix}-`));
  await writeFile(join(dir, 'README.md'), '# Caipi CLI smoke test\n\nThis is a temporary test project.\n');
  return dir;
}

function parseJsonLine(line, context) {
  try {
    return JSON.parse(line);
  } catch (error) {
    throw new Error(`${context} emitted non-JSON output: ${line.slice(0, 240)}\n${error.message}`);
  }
}

async function runClaudeSmoke() {
  const projectDir = await makeTempProject('caipi-claude-smoke');
  const model = process.env.CAIPI_CLI_SMOKE_CLAUDE_MODEL ?? 'haiku';
  const prompt = 'Reply exactly with caipi-claude-smoke-ok and do not use tools.';

  log(`Claude smoke: model=${model}`);

  const child = spawn('claude', [
    '-p',
    prompt,
    '--output-format',
    'stream-json',
    '--verbose',
    '--model',
    model,
  ], {
    cwd: projectDir,
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  let stderr = '';
  const events = [];
  let sawInit = false;
  let sawAssistantText = false;
  let sawSuccess = false;

  await new Promise((resolve, reject) => {
    const timeout = withTimeout('Claude smoke', DEFAULT_TIMEOUT_MS, (error) => {
      kill(child);
      reject(error);
    });

    child.on('error', (error) => {
      clearTimeout(timeout);
      reject(error);
    });

    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });

    const lines = createInterface({ input: child.stdout });
    lines.on('line', (line) => {
      if (!line.trim()) return;
      const event = parseJsonLine(line, 'Claude');
      events.push(event);
      sawInit ||= event.type === 'system' && event.subtype === 'init';
      sawAssistantText ||= event.type === 'assistant'
        && Array.isArray(event.message?.content)
        && event.message.content.some((block) => block.type === 'text' && block.text?.includes('caipi-claude-smoke-ok'));
      sawSuccess ||= event.type === 'result' && event.subtype === 'success';
    });

    child.on('close', (code) => {
      clearTimeout(timeout);
      if (code !== 0) {
        reject(new Error(`Claude exited with code ${code}.\n${stderr.trim()}`));
        return;
      }
      resolve();
    });
  }).finally(async () => {
    await rm(projectDir, { recursive: true, force: true });
  });

  if (!sawInit) throw new Error('Claude smoke did not receive system init event.');
  if (!sawAssistantText) throw new Error('Claude smoke did not receive expected assistant text.');
  if (!sawSuccess) throw new Error('Claude smoke did not receive success result event.');

  log(`Claude smoke passed (${events.length} events).`);
}

async function writeJsonLine(child, value) {
  child.stdin.write(`${JSON.stringify(value)}\n`);
}

async function runCodexSmoke() {
  const projectDir = await makeTempProject('caipi-codex-smoke');
  const model = process.env.CAIPI_CLI_SMOKE_CODEX_MODEL ?? 'gpt-5.3-codex';
  const prompt = 'Reply exactly with caipi-codex-smoke-ok and do not use tools.';

  log(`Codex smoke: model=${model}`);

  const child = spawn('codex', ['app-server'], {
    cwd: projectDir,
    stdio: ['pipe', 'pipe', 'pipe'],
  });

  let stderr = '';
  let nextId = 1;
  let threadId = null;
  let sawInitializeResponse = false;
  let sawThreadStartResponse = false;
  let sawTurnTerminal = false;
  let sawAssistantText = false;
  const methods = [];
  const pending = new Map();

  function request(method, params = {}) {
    const id = nextId++;
    writeJsonLine(child, { jsonrpc: '2.0', id, method, params });
    return new Promise((resolve, reject) => {
      pending.set(id, { method, resolve, reject });
    });
  }

  function notify(method, params = {}) {
    writeJsonLine(child, { jsonrpc: '2.0', method, params });
  }

  await new Promise((resolve, reject) => {
    const timeout = withTimeout('Codex smoke', DEFAULT_TIMEOUT_MS, (error) => {
      kill(child);
      reject(error);
    });

    child.on('error', (error) => {
      clearTimeout(timeout);
      reject(error);
    });

    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });

    const lines = createInterface({ input: child.stdout });
    lines.on('line', (line) => {
      if (!line.trim()) return;
      const message = parseJsonLine(line, 'Codex');

      if (message.id !== undefined && (message.result !== undefined || message.error !== undefined)) {
        const id = Number(message.id);
        const waiter = pending.get(id);
        if (waiter) {
          pending.delete(id);
          if (message.error) {
            waiter.reject(new Error(`${waiter.method} failed: ${JSON.stringify(message.error)}`));
          } else {
            waiter.resolve(message.result ?? {});
          }
        }
        return;
      }

      if (message.id !== undefined && message.method?.includes('requestApproval')) {
        writeJsonLine(child, {
          jsonrpc: '2.0',
          id: message.id,
          result: { decision: 'accept' },
        });
        return;
      }

      if (message.method) {
        methods.push(message.method);
        const params = message.params ?? {};
        sawAssistantText ||= JSON.stringify(params).includes('caipi-codex-smoke-ok');
        sawTurnTerminal ||= /turn\/(completed|failed|aborted)|codex\/event\/turn_(completed|failed|aborted)/.test(message.method);
      }
    });

    (async () => {
      try {
        await new Promise((startResolve) => setTimeout(startResolve, 50));
        await request('initialize', {
          clientInfo: {
            name: 'caipi-cli-smoke',
            version: '0.0.0',
          },
        });
        sawInitializeResponse = true;
        notify('initialized', {});

        const started = await request('thread/start', {});
        threadId = started.threadId ?? started.id ?? started.thread?.id;
        if (!threadId) {
          throw new Error(`thread/start response did not include a thread id: ${JSON.stringify(started)}`);
        }
        sawThreadStartResponse = true;

        await request('turn/start', {
          threadId,
          input: [{ type: 'text', text: prompt }],
          model,
          effort: 'low',
          approvalPolicy: 'on-request',
          sandboxPolicy: { type: 'readOnly' },
        });
      } catch (error) {
        clearTimeout(timeout);
        reject(error);
      }
    })();

    const terminalCheck = setInterval(() => {
      if (sawTurnTerminal) {
        clearInterval(terminalCheck);
        clearTimeout(timeout);
        kill(child);
        resolve();
      }
    }, 250);

    child.on('close', (code) => {
      clearInterval(terminalCheck);
      clearTimeout(timeout);
      if (!sawTurnTerminal) {
        reject(new Error(`Codex exited before terminal turn event (code ${code}).\n${stderr.trim()}`));
      }
    });
  }).finally(async () => {
    kill(child);
    await rm(projectDir, { recursive: true, force: true });
  });

  if (!sawInitializeResponse) throw new Error('Codex smoke did not receive initialize response.');
  if (!sawThreadStartResponse) throw new Error('Codex smoke did not receive thread/start response.');
  if (!sawAssistantText) throw new Error(`Codex smoke did not receive expected assistant text. Methods: ${methods.join(', ')}`);
  if (!sawTurnTerminal) throw new Error(`Codex smoke did not receive terminal turn event. Methods: ${methods.join(', ')}`);

  log(`Codex smoke passed (${methods.length} notifications).`);
}

const tasks = [];
if (selectedBackend === 'both' || selectedBackend === 'claude') tasks.push(runClaudeSmoke);
if (selectedBackend === 'both' || selectedBackend === 'codex') tasks.push(runCodexSmoke);

for (const task of tasks) {
  await task();
}

log('All selected real CLI smoke tests passed.');
