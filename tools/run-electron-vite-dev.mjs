import { chmod, mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const env = { ...process.env };
const wslg = isWslg();
const electronFlags = getElectronFlags(wslg);

if (wslg) {
  env.DISPLAY = isValidDisplay(env.DISPLAY) ? env.DISPLAY : ':0';
  env.LIBGL_ALWAYS_SOFTWARE ??= '1';
}

if (electronFlags.length > 0) {
  const electronPath = join(
    repoRoot,
    'node_modules',
    'electron',
    'dist',
    'electron',
  );
  const wrapperPath = join(
    repoRoot,
    'node_modules',
    '.cache',
    'apkrunner',
    'electron-dev',
  );
  const quotedArgs = [electronPath, ...electronFlags].map(shellQuote).join(' ');

  await mkdir(dirname(wrapperPath), { recursive: true });
  await writeFile(
    wrapperPath,
    ['#!/usr/bin/env bash', `exec ${quotedArgs} "$@"`, ''].join('\n'),
  );
  await chmod(wrapperPath, 0o755);

  env.ELECTRON_EXEC_PATH = wrapperPath;
}

const electronVite = join(
  repoRoot,
  'node_modules',
  'electron-vite',
  'bin',
  'electron-vite.js',
);
const child = spawn(process.execPath, [electronVite, 'dev', ...args], {
  cwd: join(repoRoot, 'apps', 'desktop'),
  env,
  stdio: 'inherit',
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});

function parseElectronFlags(value) {
  if (!value) {
    return [];
  }

  try {
    const parsed = JSON.parse(value);
    if (
      Array.isArray(parsed) &&
      parsed.every((item) => typeof item === 'string')
    ) {
      return parsed;
    }
  } catch {}

  return value.split(/\s+/).filter(Boolean);
}

function shellQuote(value) {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function getElectronFlags(wslg) {
  const configuredFlags = parseElectronFlags(
    process.env.APKRUNNER_ELECTRON_FLAGS,
  );
  if (configuredFlags.length > 0) {
    return configuredFlags;
  }

  return wslg
    ? ['--no-sandbox', '--in-process-gpu', '--disable-gpu-compositing']
    : [];
}

function isWslg() {
  return (
    process.platform === 'linux' &&
    Boolean(process.env.WSL_DISTRO_NAME) &&
    Boolean(process.env.WAYLAND_DISPLAY)
  );
}

function isValidDisplay(value) {
  return typeof value === 'string' && /^(:\d+|[A-Za-z0-9_.-]+:\d+)/.test(value);
}
