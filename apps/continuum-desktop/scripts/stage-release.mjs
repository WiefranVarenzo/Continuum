// Run from any directory: node stage-release.mjs <windows|linux> <target|native> <output>
// Copies complete built packages, never a bare application executable.
import { constants } from 'node:fs';
import { mkdir, readdir, readFile, copyFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const [platform, target, output, ...extra] = process.argv.slice(2);
if (extra.length || !output || !['windows', 'linux'].includes(platform)) {
  throw new Error('Usage: stage-release.mjs <windows|linux> <target|native> <output-directory>');
}
if (platform === 'windows' && !/^x86_64-pc-windows-(msvc|gnu)$/.test(target ?? '')) {
  throw new Error('Windows packages require an explicit supported x64 target.');
}
if (platform === 'linux' && target !== 'native') {
  throw new Error('This Linux release uses the native x86-64 builder.');
}
if (platform === 'linux' && process.arch !== 'x64') {
  throw new Error('Linux release staging is qualified only for an x86-64 builder.');
}
const app = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const config = JSON.parse(await readFile(path.join(app, 'src-tauri/tauri.conf.json'), 'utf8'));
const version = config.version;
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) throw new Error('Invalid release version');
const bundle = path.join(app, 'src-tauri/target', target === 'native' ? '' : target, 'release/bundle');
const destination = path.resolve(output);
const selections = platform === 'windows'
  ? [{folder: 'nsis', ending: '-setup.exe', name: `Continuum_${version}_windows_x64-setup.exe`}]
  : [{folder: 'appimage', ending: '.AppImage', name: `Continuum_${version}_linux_x86_64.AppImage`},
     {folder: 'deb', ending: '.deb', name: `Continuum_${version}_linux_amd64.deb`}];
const packages = [];
for (const selection of selections) {
  const folder = path.join(bundle, selection.folder);
  const matches = (await readdir(folder, {withFileTypes: true})).filter(entry =>
    entry.isFile() && entry.name.includes(`_${version}_`) && entry.name.endsWith(selection.ending));
  if (matches.length !== 1) throw new Error(`Expected one ${selection.folder} package for ${version}, found ${matches.length}`);
  const source = path.join(folder, matches[0].name);
  const data = await readFile(source);
  if (!data.length) throw new Error(`Empty package: ${source}`);
  packages.push({source, name: selection.name, sha256: createHash('sha256').update(data).digest('hex'), bytes: data.length});
}
await mkdir(destination, {recursive: true});
for (const item of packages) await copyFile(item.source, path.join(destination, item.name), constants.COPYFILE_EXCL);
await writeFile(path.join(destination, `SHA256SUMS-${platform}.txt`),
  packages.map(item => `${item.sha256}  ${item.name}`).join('\n') + '\n', {flag: 'wx'});
console.log(JSON.stringify({version, platform, target, packages: packages.map(({source, ...item}) => item)}, null, 2));
