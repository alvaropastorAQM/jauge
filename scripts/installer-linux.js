/* Builds Jauge for Linux and installs it for the current user (npm run install-app:linux): the executable and the
   Claude binary of the SDK side by side in ~/.local/opt/jauge, plus a .desktop entry, which WSLg also lists in the
   Windows Start menu. */
'use strict';
const { execFileSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const racine = path.join(__dirname, '..');
const tauri = path.join(racine, 'src-tauri');
const lancer = (commande, args, options = {}) => execFileSync(commande, args, { cwd: racine, stdio: 'inherit', ...options });
const arch = { arm64: 'arm64', x64: 'x64' }[process.arch];
if (!arch) throw new Error('Unsupported architecture: ' + process.arch);

lancer('node', ['tests/test.js']);
lancer('cargo', ['test', '--release', '--quiet'], { cwd: tauri });
lancer('cargo', ['build', '--release'], { cwd: tauri });

const cible = path.join(os.homedir(), '.local/opt/jauge');
const executable = path.join(cible, 'jauge');
const nouvelle = cible + '.nouvelle';
fs.rmSync(nouvelle, { recursive: true, force: true });
fs.mkdirSync(nouvelle, { recursive: true });
fs.copyFileSync(path.join(tauri, 'target/release/jauge'), path.join(nouvelle, 'jauge'));
fs.copyFileSync(path.join(racine, `node_modules/@anthropic-ai/claude-agent-sdk-linux-${arch}/claude`), path.join(nouvelle, 'claude'));
fs.chmodSync(path.join(nouvelle, 'jauge'), 0o755);
fs.chmodSync(path.join(nouvelle, 'claude'), 0o755);
fs.copyFileSync(path.join(tauri, 'icons/icon.png'), path.join(nouvelle, 'icon.png'));

/* A build that does not start is never installed. */
const essai = fs.mkdtempSync(path.join(os.tmpdir(), 'jauge-essai-'));
lancer(path.join(nouvelle, 'jauge'), [], { env: { ...process.env, JAUGE_ESSAI: essai, JAUGE_DEMO: '1' }, timeout: 60000, stdio: 'ignore' });
fs.rmSync(essai, { recursive: true, force: true });

try { execFileSync('pkill', ['-x', 'jauge']); } catch { /* it was not running */ }
fs.rmSync(cible, { recursive: true, force: true });
fs.renameSync(nouvelle, cible);

const applications = path.join(os.homedir(), '.local/share/applications');
fs.mkdirSync(applications, { recursive: true });
fs.writeFileSync(path.join(applications, 'jauge.desktop'), [
  '[Desktop Entry]',
  'Type=Application',
  'Name=Jauge',
  'Comment=Your Claude limits, live',
  'Exec="' + executable + '"',
  'Icon=' + path.join(cible, 'icon.png'),
  'Categories=Utility;',
  'StartupWMClass=jauge',
  '',
].join('\n'));
console.log('Installed: ' + executable);
