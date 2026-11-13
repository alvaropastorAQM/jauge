/* Builds Jauge.app (Tauri) and places it in /Applications (npm run install-app). */
'use strict';
const { execFileSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const racine = path.join(__dirname, '..');
const tauri = path.join(racine, 'src-tauri');
const cible = '/Applications/Jauge.app';
const lancer = (commande, args, options = {}) => execFileSync(commande, args, { cwd: racine, stdio: 'inherit', ...options });
const version = JSON.parse(fs.readFileSync(path.join(tauri, 'tauri.conf.json'), 'utf8')).version;

lancer('node', ['tests/test.js']);
lancer('cargo', ['test', '--release', '--quiet'], { cwd: tauri });
lancer('cargo', ['build', '--release'], { cwd: tauri });

/* The bundle, by hand: the executable, the Claude binary shipped with the SDK, the icon and the Info.plist. */
const construite = path.join(racine, 'release', 'Jauge.app');
const contenu = path.join(construite, 'Contents');
fs.rmSync(construite, { recursive: true, force: true });
fs.mkdirSync(path.join(contenu, 'MacOS'), { recursive: true });
fs.mkdirSync(path.join(contenu, 'Resources'));
fs.copyFileSync(path.join(tauri, 'target/release/jauge'), path.join(contenu, 'MacOS/Jauge'));
fs.chmodSync(path.join(contenu, 'MacOS/Jauge'), 0o755);
/* ditto keeps Anthropic's signature on the Claude binary, which the keychain ties the sign-in to. */
lancer('ditto', [path.join(racine, `node_modules/@anthropic-ai/claude-agent-sdk-darwin-${process.arch}/claude`), path.join(contenu, 'Resources/claude')]);
fs.copyFileSync(path.join(tauri, 'icons/icon.icns'), path.join(contenu, 'Resources/icon.icns'));
fs.writeFileSync(path.join(contenu, 'Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>Jauge</string>
  <key>CFBundleExecutable</key><string>Jauge</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundleIdentifier</key><string>com.example.jauge</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Jauge</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <!-- No Dock icon, not even for a second at launch. -->
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
`);

/* Local signature of the app alone, without --deep: it would replace the signature of the Claude binary. */
lancer('codesign', ['--force', '--sign', '-', construite]);
lancer('codesign', ['--verify', '--strict', construite]);
lancer('codesign', ['--verify', path.join(contenu, 'Resources/claude')]);

/* A build that does not start is never installed. */
const essai = fs.mkdtempSync(path.join(os.tmpdir(), 'jauge-essai-'));
lancer(path.join(contenu, 'MacOS/Jauge'), [], { env: { ...process.env, JAUGE_ESSAI: essai, JAUGE_DEMO: '1' }, timeout: 60000, stdio: 'ignore' });
fs.rmSync(essai, { recursive: true, force: true });

/* The old version may be running: quit it before the swap. */
try { execFileSync('pkill', ['-x', 'Jauge']); } catch { /* it was not running */ }

/* The copy is ready alongside before touching the app: the swap is two renames. */
const nouvelle = cible + '.nouvelle', ancienne = cible + '.ancienne';
fs.rmSync(nouvelle, { recursive: true, force: true });
fs.rmSync(ancienne, { recursive: true, force: true });
lancer('ditto', [construite, nouvelle]);
lancer('codesign', ['--verify', '--strict', nouvelle]);
if (fs.existsSync(cible)) fs.renameSync(cible, ancienne);
fs.renameSync(nouvelle, cible);
fs.rmSync(ancienne, { recursive: true, force: true });
console.log('Installed: ' + cible);
/* Right after the swap, macOS may not have finished registering the new app: try again a little later. */
for (let essai = 1; ; essai++) {
  try { lancer('open', [cible]); break; } catch (e) {
    if (essai === 5) throw e;
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1000);
  }
}
