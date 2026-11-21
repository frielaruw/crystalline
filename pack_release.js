// Stages only ship files into dist_src/, then packages a portable win32 build.
// Avoids electron-packager --ignore regex pitfalls — the folder IS the manifest.
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const root = __dirname;
const stage = path.join(root, 'dist_src');

const SHIP_FILES = [
  'package.json',
  'electron/main.js',
  'electron/preload.js',
  'renderer/index.html',
  'renderer/renderer.js',
  'renderer/styles.css',
  'tweaks/catalog.json',
  'native-engine/crystalline-engine.exe',
];

for (const rel of SHIP_FILES) {
  const src = path.join(root, rel);
  if (!fs.existsSync(src)) {
    console.error('MISSING ship file:', rel);
    process.exit(1);
  }
}

fs.rmSync(stage, { recursive: true, force: true });
for (const rel of SHIP_FILES) {
  const dst = path.join(stage, rel);
  fs.mkdirSync(path.dirname(dst), { recursive: true });
  fs.copyFileSync(path.join(root, rel), dst);
}
console.log(`Staged ${SHIP_FILES.length} files -> dist_src/`);

cp.execSync(
  'npx electron-packager dist_src Crystalline --platform=win32 --arch=x64 --out=dist --overwrite',
  { cwd: root, stdio: 'inherit' }
);
console.log('Done -> dist/Crystalline-win32-x64');
