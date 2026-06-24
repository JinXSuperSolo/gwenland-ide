# Release Build Size Optimization and Compression

- **Date:** 2026-06-17
- **Issue:** GWEN-225
- **Milestone:** Milestone 3 — Release Optimization

## Problem / Context

Kondisi awal biner rilis hasil kompilasi standar memiliki ukuran yang relatif besar (sekitar 10-15 MB). Hal ini kurang optimal untuk GwenLand IDE yang didesain agar sangat ringan, efisien, dan memiliki footprint instalasi sekecil mungkin. Diperlukan optimasi profil kompilasi rilis di level kargo (Cargo workspace) untuk mereduksi ukuran biner secara maksimal.

## Change

Melakukan modifikasi pada file konfigurasi:
- `Cargo.toml` (Root Workspace): Menambahkan konfigurasi profil `[profile.release]` dengan opsi:
  - `opt-level = "z"` (optimasi penuh untuk ukuran biner minimum).
  - `lto = true` (mengaktifkan Link-Time Optimization lintas crate).
  - `codegen-units = 1` (membatasi unit codegen menjadi satu untuk efisiensi optimasi optimal).
  - `panic = "abort"` (mengganti mekanisme panik dengan abort untuk memangkas tabel stack unwinding).
  - `strip = true` (memotong seluruh debug symbols dan tabel simbol dari biner akhir).

## Why this approach

Pendekatan ini dipilih karena secara langsung memotong metadata dan overhead biner Rust tanpa mengubah alur logika fungsionalitas program:
- `opt-level = "z"` memprioritaskan reduksi ukuran instruksi assembly dibanding kecepatan eksekusi mentah.
- Mengurangi codegen-units ke `1` dan mengaktifkan LTO penuh memberikan kompilator pandangan holistik terhadap seluruh codebase untuk melakukan inline fungsi dan eliminasi dead-code secara menyeluruh.
- Menghapus stack unwinding (`panic = "abort"`) memangkas kode penanganan panic yang cukup memakan tempat di biner Rust.
- Opsi `strip = true` memotong tabel simbol debugging sehingga biner bersih dari metadata yang tidak diperlukan untuk deployment produksi.

## Impact

- Hasil kompilasi biner `frontend.exe` berhasil ditekan hingga hanya **2.76 MB** (2,896,896 bytes).
- Waktu kompilasi rilis meningkat (sekitar 4-6 menit karena pengerjaan LTO penuh), namun ukuran biner menyusut drastis hingga lebih dari 75%.
- Tidak ada breaking change terhadap Tauri runtime maupun engine crate.
