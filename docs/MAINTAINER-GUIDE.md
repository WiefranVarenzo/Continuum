# Memelihara, memperbaiki, dan merilis Continuum

Panduan operasional ini melengkapi [handover arsitektur](HANDOVER.md), bukan menggantikan kontrak detail di dokumen CP/ADR. Gunakan branch/commit terpisah untuk perubahan berikutnya dan catat perilaku sebelum/sesudah secara terukur.

## Prasyarat dan setup

- Rust/Cargo dan Node.js 22 + npm. Pertahankan `Cargo.lock` dan `apps/continuum-desktop/package-lock.json` di Git; lakukan `npm ci`, bukan `npm install`, untuk setup yang reproduktif.
- Build Tauri Linux memerlukan paket pengembangan GTK/WebKit serta dukungan GStreamer; capture layar menggunakan portal/PipeWire. Build/tes Windows harus dilakukan pada Windows x64 dengan WebView2, Microsoft C++ Build Tools, Rust MSVC, dan Git for Windows. Persyaratan rinci ada di [Windows release guide](windows/WINDOWS-RELEASE.md).
- `target/`, `node_modules/`, `dist/`, bundel OS, dan proyek pengguna bukan source. Jangan commit. Pada disk kecil, arahkan `CARGO_TARGET_DIR` ke drive yang cukup besar sebelum membangun.

```bash
cd apps/continuum-desktop
npm ci
npm run tauri dev
```

## Pengujian sebelum commit

```bash
cargo fmt --all --check
cargo test --workspace
cd apps/continuum-desktop
npm test
npm run build
```

Tambahkan `cargo clippy --workspace --all-targets -- -D warnings` untuk lint penuh. Jalankan pula tes khusus area yang diubah dan uji manual pada **proyek contoh non-sensitif**. Beberapa uji Git LFS bersifat opt-in; lihat test pada `remote_sync.rs`. Jangan menafsirkan test unit sebagai bukti portal layar, mikrofon, video playback, OAuth, atau paket installer benar pada OS pengguna.

## Peta diagnosis cepat

| Gejala | Periksa lebih dulu | Jangan lakukan |
| --- | --- | --- |
| Rekaman hitam/tanpa suara/crash | Izin portal, pilihan sumber, preview input vs hasil tersimpan, codec/GStreamer, log sesi native; tes screen/mic/system audio terpisah lalu gabungan. | Menyebutnya sekadar masalah izin bila stream diberikan tetapi encoder gagal. |
| Screenshot/media gagal dibuka | Ukuran artifact, pembacaan bertahap, MIME, bytes original, media URL dan decoder; pastikan database serta folder artifact dipindah bersama. | Mengubah original media untuk membuat preview tampak benar. |
| Chat gagal atau checkpoint stale | Refresh state/context pack dan periksa status MCP grant serta hasil tool call nyata. | Mengulang pesan sambil mengasumsikan klien terhubung hanya dari konfigurasi. |
| AI/diagram/report mengarang | Telusuri sumber, status verified/candidate, prompt boundary, sanitasi Markdown/Mermaid, dan review gate. | Menaikkan proposal AI menjadi fakta otomatis. |
| Upload GitHub tampak beku | Cek apakah network/LFS masih berjalan, apakah UI thread/app lock bebas, branch remote, serta commit/link lokal terakhir. | Force-push atau import remote ke proyek kerja sebagai langkah pemulihan otomatis. |
| Proyek lain tidak muncul | Cari folder proyek asli; daftar recent hanya petunjuk lokal perangkat. Buka folder itu manual atau import snapshot lengkap. | Menganggap source repo adalah backup semua data pengguna. |

## Aturan perubahan data

Perubahan skema ledger memerlukan migration baru dan tes upgrade dari fixture lama. Jangan edit migration yang telah dirilis untuk “memperbaiki” database pengguna. Artifact asli, referensi ID, relasi, dan audit/provenance harus tetap valid. Export/import wajib memverifikasi hasil dan tidak boleh menimpa folder yang sudah ada. Saat memperbaiki laporan atau tampilan workspace, jangan memodifikasi fakta kanonik hanya demi tampilan.

## Packaging dan release

- Versi Linux yang terakhir divalidasi secara lokal ada pada [dokumen follow-up 1 Oktober](cp12/2026-10-01-GITHUB-UPLOAD-RESPONSIVENESS.md). Nomor versi di Tauri/package tetap `0.12.0`; folder follow-up membedakan build. Bila membuat rilis baru, beri identitas build yang jelas dan SHA-256 baru. Jangan mengganti paket lama tanpa penjelasan.
- AppImage adalah **asset rilis**, bukan file Git biasa (ukurannya lebih dari batas file GitHub biasa). Sertakan SHA-256, release notes, matriks uji, dan peringatan bahwa paket belum ditandatangani. Jangan unggah seluruh `releases/` historis ke riwayat source.
- Workflow `.github/workflows/windows-desktop.yml` dapat membangun installer Windows **unsigned**, tetapi hasil CI belum membuktikan capture/audio, integrasi AI, atau pengalaman instalasi. Ikuti [acceptance test Windows](windows/WINDOWS-RELEASE.md) sebelum menyebutnya siap dipakai.
- macOS belum mempunyai release gate lengkap; jangan membuat klaim kompatibilitas hanya karena UI berbasis React/Tauri.

## Handover proyek/data ke orang lain

1. Bagikan source repo ini untuk kode, spesifikasi, tes, dan riwayat perubahan.
2. Jika perlu contoh data, buat proyek uji non-sensitif lalu ekspor **proyek lengkap** atau snapshot ke repo privat data terpisah. Media disimpan melalui Git LFS; orang penerima memerlukan Git/Git LFS dan akses repo.
3. Penerima import ke folder baru, jalankan pemeriksaan integritas, cek workspace layout, evidence playback, report, chat, dan bookmark. Baru kemudian hubungkan ulang Git serta AI client setempat. Jangan berbagi token, project pribadi, atau source `.env`.
4. Catat perubahan terakhir, tes yang dijalankan, tes yang belum dijalankan, dan limitasi terbuka di dokumen follow-up baru agar AI berikutnya tidak membaca status lama sebagai status baru.
