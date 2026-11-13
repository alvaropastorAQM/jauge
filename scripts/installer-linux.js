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

/* Under WSL, WSLg does not always publish ~/.local/share/applications to the Windows Start menu: a Windows shortcut
   launches Jauge through wslg.exe (no console window), with the icon converted to .ico by PowerShell. */
const distribution = process.env.WSL_DISTRO_NAME;
if (distribution) {
  const icone = execFileSync('wslpath', ['-w', path.join(cible, 'icon.png')], { encoding: 'utf8' }).trim();
  const ps = (s) => "'" + s.replace(/'/g, "''") + "'";
  const script = [
    'Add-Type -AssemblyName System.Drawing',
    '$dossier = Join-Path $env:LOCALAPPDATA "Jauge"; New-Item -ItemType Directory -Force $dossier | Out-Null',
    '$ico = Join-Path $dossier "jauge.ico"',
    '$source = [System.Drawing.Image]::FromFile(' + ps(icone) + ')',
    '$image = New-Object System.Drawing.Bitmap $source, 256, 256; $source.Dispose()',
    '$png = New-Object System.IO.MemoryStream; $image.Save($png, [System.Drawing.Imaging.ImageFormat]::Png); $image.Dispose()',
    '$o = $png.ToArray(); $f = [System.IO.File]::Create($ico); $w = New-Object System.IO.BinaryWriter $f',
    // ICO header, one 256x256 entry (0 means 256), 32 bits, PNG payload right after the 22-byte header.
    '$w.Write([UInt16]0); $w.Write([UInt16]1); $w.Write([UInt16]1); $w.Write([Byte]0); $w.Write([Byte]0); $w.Write([Byte]0); $w.Write([Byte]0)',
    '$w.Write([UInt16]1); $w.Write([UInt16]32); $w.Write([UInt32]$o.Length); $w.Write([UInt32]22); $w.Write($o); $w.Close()',
    '$lnk = Join-Path ([Environment]::GetFolderPath("Programs")) "Jauge.lnk"',
    '$s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)',
    '$s.TargetPath = Join-Path $env:ProgramFiles "WSL\\wslg.exe"',
    '$s.Arguments = ' + ps('-d ' + distribution + ' --cd ~ -- ' + executable),
    '$s.IconLocation = $ico; $s.Description = "Your Claude limits, live"; $s.Save()',
    'Write-Output $lnk',
  ].join('; ');
  const lnk = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], { cwd: '/mnt/c', encoding: 'utf8' }).trim();
  console.log('Start menu: ' + lnk);
}
