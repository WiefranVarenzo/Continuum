# Continuum workspace candidate — 25 September 2026

Paket ini adalah **kandidat uji**, bukan klaim seluruh strategi telah selesai atau bebas bug. Paket lama di `releases/0.12.0` tidak ditimpa.

## Menjalankan

Tutup versi Continuum sebelumnya, lalu buka `Continuum_0.12.0_amd64.AppImage` di folder ini. Jangan menjalankan dua versi berbeda pada proyek yang sama.

Untuk percobaan pertama, gunakan proyek uji atau ekspor salinan proyek melalui versi lama. Database versi 14 akan diperbarui ke versi 15; sebelum migrasi, core membuat backup database di folder `backups` proyek. Jangan membuka kembali database yang sudah diperbarui menggunakan binary lama.

## Alur yang bisa dicoba

1. Buka/buat riset. Tentukan judul, pertanyaan dan konteksnya.
2. Paste/drop gambar. Judul dan deskripsi diminta setelah original tersimpan. Media audio/video yang didukung dapat dibuka dari kartu, dengan batas impor 32 MiB dan ketergantungan codec OS.
3. Geser kartu dan tarik konektor untuk menghubungkan. Klik garis untuk mengubah label/alasan atau menghapusnya. Gunakan Undo/Redo untuk membatalkan/mengembalikan perubahan. Auto layout terpisah dari analisis AI agar posisi manual tidak berubah diam-diam.
4. Pilih **Codex** atau **Hermes** di header. Keduanya memerlukan CLI dan login/provider yang bekerja. Analyze & connect menghasilkan interpretasi draf dan saran garis; ini bukan verifikasi fakta otomatis. MCP untuk aplikasi AI eksternal tetap berada di AI connections, terpisah dari runner analisis dalam aplikasi.
5. Buka **Reports**. Generate report with AI membuat draf baru untuk dipilih, bukan langsung menimpa laporan. Edit Markdown, periksa sumbernya, lalu ekspor `.md` atau HTML. Diagram Mermaid dirender lokal dan memiliki zoom/fit/pan/fullscreen.
6. Gunakan **Messages** untuk bertanya tentang konteks tersimpan. Pertanyaan dan jawaban disimpan per proyek/riset. AI tidak otomatis memperoleh seluruh sumber: paket konteks tetap berbatas dan dapat diperiksa.
7. Tekan **Save bookmark** atau buka **Resume & memory** untuk menambahkan penanda dan melihat konteks serta hasil AI terakhir. Penutupan menunggu simpan/checkpoint; hentikan capture dan selesaikan dialog evidence terlebih dahulu.

## Sudah diperiksa

- 44 tes UI, 5 tes core penyimpanan/upgrade/release, 1 tes native validasi keluaran AI.
- Kompilasi produksi dan pengemasan Linux AppImage.
- Preview browser dengan data sintetis: board, auto layout/undo, edit/hapus garis dan pemulihannya, Messages, serta render/zoom Mermaid.
- HTML ekspor diperiksa melalui tes lokal. Navigasi blob ekspor pada browser otomasi diblokir; tidak diklaim telah diperiksa secara visual di Chrome/native.
- Pada 26 September, AppImage berhasil menyala 15 detik dengan pengaturan sementara terpisah; uji ini tidak membuka proyek pribadi. Satu permintaan Codex CLI dengan login ChatGPT dan pengaturan isolasi yang dipakai adapter AI menghasilkan JSON sesuai format. Ini belum menguji tombol AI dalam aplikasi dengan data proyek sungguhan.

## Masih perlu pilot nyata

- Login/model Codex dan Hermes serta analisis gambar sungguhan belum disertifikasi oleh tes UI sintetis. Batas saat ini: Codex maksimal 8 preview gambar; Hermes maksimal 1 sesuai antarmuka CLI yang terpasang. Transkripsi audio/video belum ditambahkan.
- Dalam satu uji Codex CLI yang sangat singkat, CLI melaporkan 27.537 token terpakai. Angka ini bukan estimasi tetap per permintaan, tetapi alasan untuk menjalankan analisis saat dibutuhkan dan memeriksa penggunaan akun. Hermes terdeteksi memiliki kredensial, namun respons model Hermes belum diuji.
- Runner memakai konfigurasi CLI terisolasi/safe defaults, bukan seluruh custom hook/model profile/MCP dari konfigurasi pribadi. Retensi data mengikuti agent/provider yang dipakai; penghapusan gambar sementara tidak berarti penghapusan riwayat provider.
- Izin screen/audio, codec media, serta pengujian perangkat nyata tetap diperlukan. Windows/macOS belum dibangun atau diuji pada iterasi ini.
- Dokumen dibatasi 2 MiB; histori board 40 langkah. Penyimpanan percakapan besar dan stress test jangka panjang belum menjadi klaim rilis ini.
- Jika MCP eksternal masih menunjuk server lama, gunakan Check setup/reconnect lalu restart client untuk memperbarui binary server yang sesuai dengan schema 15.

Catatan implementasi lengkap: `docs/cp12/2026-09-25-WORKSPACE-CONTINUITY-PLAN.md` di repository.
