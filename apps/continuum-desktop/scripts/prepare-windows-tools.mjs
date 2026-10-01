// Official upstream portable distributions, pinned and SHA-256 verified.
// No installer or global PATH/configuration changes are needed.
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile, copyFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const tools = path.join(repo, 'packaging/windows/tools');
const cache = path.join(repo, 'packaging/windows-downloads');
const distributions = [
  {name:'git', url:'https://github.com/git-for-windows/git/releases/download/v2.56.0.windows.1/MinGit-2.56.0-64-bit.zip',
    sha256:'064b440ff870ed5198527e8f3a92cdf5bd2fd0fedf5e718af95e3fdaddeff718'},
  {name:'lfs', url:'https://github.com/git-lfs/git-lfs/releases/download/v3.8.0/git-lfs-windows-amd64-v3.8.0.zip',
    sha256:'b62e7b8ceddee635f691233d77de8eaa4b213e9209e0173811d8cfa77f7882c1'},
  {name:'gh', url:'https://github.com/cli/cli/releases/download/v2.102.0/gh_2.102.0_windows_amd64.zip',
    sha256:'ae64e556ecc240b200f7eba60d550e4bb60d78e860e69dd88c449405b86067f4'},
];
await mkdir(cache, {recursive:true});
await mkdir(tools, {recursive:true});
const quote = value => `'${value.replaceAll("'", "''")}'`;
for (const distribution of distributions) {
  const archive = path.join(cache, `${distribution.name}.zip`);
  if (!existsSync(archive)) {
    const response = await fetch(distribution.url);
    if (!response.ok) throw new Error(`Cannot download ${distribution.name}: ${response.status}`);
    await writeFile(archive, Buffer.from(await response.arrayBuffer()));
  }
  const digest = createHash('sha256').update(await readFile(archive)).digest('hex');
  if (digest !== distribution.sha256) throw new Error(`Checksum mismatch for ${distribution.name}; refusing to package it.`);
  const destination = path.join(tools, distribution.name);
  if (!existsSync(path.join(destination, '.verified'))) {
    const result = process.platform === 'win32'
      ? spawnSync('powershell.exe', ['-NoProfile','-NonInteractive','-Command',
        `Add-Type -AssemblyName System.IO.Compression.FileSystem; [System.IO.Compression.ZipFile]::ExtractToDirectory(${quote(archive)}, ${quote(destination)})`], {stdio:'inherit'})
      : spawnSync('unzip', ['-q', archive, '-d', destination], {stdio:'inherit'});
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error(`Cannot extract ${distribution.name}`);
    await writeFile(path.join(destination, '.verified'), digest);
  }
}
await copyFile(path.join(tools, 'lfs/git-lfs-3.8.0/git-lfs.exe'), path.join(tools, 'git/ucrt64/bin/git-lfs.exe'));
await writeFile(path.join(tools, 'UPSTREAM.json'), JSON.stringify(distributions, null, 2));
console.log('Portable Git, Git LFS and GitHub CLI verified and prepared.');
