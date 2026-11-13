/* Construit Jauge.app (Tauri) et la place dans /Applications (npm run install-app). */
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

/* Le bundle, à la main : l'exécutable, le binaire de Claude livré avec le SDK, l'icône et l'Info.plist. */
const construite = path.join(racine, 'release', 'Jauge.app');
const contenu = path.join(construite, 'Contents');
fs.rmSync(construite, { recursive: true, force: true });
fs.mkdirSync(path.join(contenu, 'MacOS'), { recursive: true });
fs.mkdirSync(path.join(contenu, 'Resources'));
fs.copyFileSync(path.join(tauri, 'target/release/jauge'), path.join(contenu, 'MacOS/Jauge'));
fs.chmodSync(path.join(contenu, 'MacOS/Jauge'), 0o755);
/* ditto garde la signature d'Anthropic du binaire de Claude, à laquelle le trousseau rattache la connexion. */
lancer('ditto', [path.join(racine, `node_modules/@anthropic-ai/claude-agent-sdk-darwin-${process.arch}/claude`), path.join(contenu, 'Resources/claude')]);
fs.copyFileSync(path.join(tauri, 'icons/icon.icns'), path.join(contenu, 'Resources/icon.icns'));
fs.writeFileSync(path.join(contenu, 'Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>fr</string>
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
  <!-- Pas d'icône dans le Dock, même une seconde au lancement. -->
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
`);

/* Signature locale de l'app seule, sans --deep : elle remplacerait celle du binaire de Claude. */
lancer('codesign', ['--force', '--sign', '-', construite]);
lancer('codesign', ['--verify', '--strict', construite]);
lancer('codesign', ['--verify', path.join(contenu, 'Resources/claude')]);

/* Une version qui ne démarre pas n'est jamais installée. */
const essai = fs.mkdtempSync(path.join(os.tmpdir(), 'jauge-essai-'));
lancer(path.join(contenu, 'MacOS/Jauge'), [], { env: { ...process.env, JAUGE_ESSAI: essai, JAUGE_DEMO: '1' }, timeout: 60000, stdio: 'ignore' });
fs.rmSync(essai, { recursive: true, force: true });

/* L'ancienne version tourne peut-être : on la quitte avant l'échange. */
try { execFileSync('pkill', ['-x', 'Jauge']); } catch { /* elle ne tournait pas */ }

/* La copie est prête à côté avant de toucher à l'app : l'échange tient en deux renommages. */
const nouvelle = cible + '.nouvelle', ancienne = cible + '.ancienne';
fs.rmSync(nouvelle, { recursive: true, force: true });
fs.rmSync(ancienne, { recursive: true, force: true });
lancer('ditto', [construite, nouvelle]);
lancer('codesign', ['--verify', '--strict', nouvelle]);
if (fs.existsSync(cible)) fs.renameSync(cible, ancienne);
fs.renameSync(nouvelle, cible);
fs.rmSync(ancienne, { recursive: true, force: true });
console.log('Installé : ' + cible);
/* Juste après l'échange, macOS peut ne pas avoir fini d'enregistrer la nouvelle app : on réessaie un peu plus tard. */
for (let essai = 1; ; essai++) {
  try { lancer('open', [cible]); break; } catch (e) {
    if (essai === 5) throw e;
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1000);
  }
}
