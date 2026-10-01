# Continuum — kandidat perbaikan 28 September 2026

Ukuran AppImage: 120.531.448 byte (sekitar 115 MiB). Checksum berada di `SHA256SUMS`. Rilis sebelumnya tidak ditimpa. Kandidat ini bukan klaim seluruh fitur sudah bebas bug.

## Yang berubah

- Izin layar/mikrofon di WebKitGTK sekarang ditangani dengan dialog Allow/Cancel. Permintaan layar langsung berasal dari klik, tidak tertunda oleh penyimpanan. Ada pembatalan dan batas waktu; stream yang terlambat disetujui dibersihkan.
- System audio di Linux dapat memakai monitor keluaran PipeWire/PulseAudio melalui `parec`, yang tersedia pada Fedora ini. Perekaman tetap memerlukan tindakan dan persetujuan pengguna. Jalur audio dibatasi dan dihentikan bila macet.
- Hasil rekaman dapat diberi judul/catatan dan disatukan menjadi satu kartu workspace yang bisa diputar. Potongan asli tetap disimpan; rekaman tersimpan dapat ditinjau kembali setelah membuka aplikasi. Batas hasil gabungan saat ini 64 MiB. Rekaman yang lebih besar tidak dibuang, tetapi belum bisa disatukan melalui fitur ini.
- Reports baru benar-benar dimulai dari Markdown kosong. AI bebas menyusun strukturnya; template lama hanya referensi/import opsional. Laporan lama tidak dihapus.
- Draft Markdown dari klien MCP dapat dipratinjau dan dipilih lewat **Reports → Use a connected AI through MCP**. **Generate report with AI** merupakan jalur berbeda melalui CLI Codex/Hermes yang dipilih di header. MCP terpasang tidak otomatis menjalankan model.
- Asal draft diperjelas. Laporan tidak ditimpa tanpa pilihan pengguna. Mermaid menyesuaikan ukuran panel dan tetap memiliki zoom/pan/fit/fullscreen. Zoom manual tidak dibatalkan oleh perubahan ukuran biasa.

## Cara mencoba

Tutup Continuum versi lama. Buka AppImage di folder ini. Jangan membuka proyek yang sama secara bersamaan di dua versi.

Untuk tes pertama, buat proyek uji Research. Versi ini tidak menambah migrasi di atas schema 15; proyek lebih lama masih mengikuti migrasi dan backup bawaan. Simpan backup/ekspor proyek sebelum memakai build percobaan untuk pekerjaan penting.

1. Buka **Record & sources**, pilih **Screen** saja, centang persetujuan, lalu **Start capture**.
2. Pilih **Allow this capture** pada dialog Continuum, kemudian pilih layar/jendela pada pemilih desktop.
3. Rekam 10 detik, tekan **Stop**, isi **Recording title** dan catatan bila perlu, lalu **Add complete recording to workspace**.
4. Buka kartu hasilnya di **Workspace** dan putar videonya. Ulangi untuk mikrofon saja, system audio saja, lalu kombinasi yang kamu butuhkan.
5. Untuk report: pilih **Generate report with AI** jika CLI sudah login, atau kirim **Copy task for connected AI** ke klien MCP yang telah tersambung dan diizinkan mengirim proposal. Setelah AI mengirim draft, tekan **Refresh incoming drafts → Preview draft → Use this draft**.
6. Periksa isi/sumber, edit Markdown bila perlu, lalu ekspor `.md` atau HTML. HTML berisi hasil render, bukan sekadar kode Markdown.

## Bukti pemeriksaan dan batasnya

- Suite UI: 53 tes lulus, kemudian 13 tes terarah lulus setelah penambahan satu regresi ukuran diagram (54 tes UI unik keseluruhan).
- Seluruh 11 tes core capture lulus; satu integrasi proposal Markdown MCP lulus; 6 tes native lulus. Tes nyata model organizer sengaja tidak dijalankan ulang.
- Build frontend/native dan pemaketan AppImage berhasil. Pemeriksaan visual browser memakai data sintetis, bukan data proyek pengguna.
- Probe suara sistem Fedora menerima satu paket 9.600 byte. Suara tidak disimpan atau dikirim. Ini memvalidasi sumber audio lokal, belum seluruh alur rekam aplikasi.
- Startup AppImage dengan konfigurasi/data/cache sementara bertahan selama 12 detik tanpa pesan error, lalu dihentikan oleh timeout uji (exit 124 yang diharapkan, bukan crash). Tidak membuka proyek pribadi. Checksum hasil salinan rilis juga lulus.
- Pemilih layar native dan playback hasil rekam nyata masih perlu percobaan pengguna. Pengujian otomatis tidak menggantikan persetujuan OS maupun pengujian perangkat sebenarnya.
- AI belum otomatis mentranskripsi audio/video. Judul, deskripsi, dan metadata tersedia sebagai konteks; jangan menganggap AI sudah mendengar seluruh rekaman.
- Windows/macOS serta respons model Hermes dan alur model penuh dalam aplikasi belum disertifikasi pada iterasi ini.

Catatan lengkap: `docs/cp12/2026-09-28-CAPTURE-MARKDOWN-HANDOFF.md` di repository.
