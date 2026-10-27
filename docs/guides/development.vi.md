# Phát triển Kakehashi trên Windows

Giao diện React/TypeScript ở `src/`; backend Tauri/Rust ở `src-tauri/src/`. Dùng pnpm, giữ `pnpm-lock.yaml` và `Cargo.lock`.

```powershell
pnpm install --frozen-lockfile
pnpm models:setup
pnpm desktop:dev
```

`scripts/desktop.ps1` dùng toolchain `stable-x86_64-pc-windows-gnu`. Cần Rust/rustup, MinGW có g++/windres và Python/pip; script kiểm tra/chuẩn bị CMake/Ninja/libclang portable trong `.native-tools/`. `scripts/native-env.ps1` thiết lập môi trường của tiến trình build.

Các lệnh desktop mặc định chuẩn bị Vulkan SDK trong `.native-tools/vulkan` khi có Vulkan loader của driver Windows. SDK được giải nén cục bộ, không chạy installer hay sửa driver. Whisper chọn GPU rời theo loại thiết bị; AVX2/FMA/F16C/BMI2 cho CPU dự phòng chỉ bật khi CPU build hỗ trợ. Runtime C++ GNU được liên kết static để tránh lỗi TLS/`std::call_once` khi suy luận Vulkan trong Rust.

Build chỉ CPU, portable: `pnpm desktop:build -WhisperBackend cpu -CpuProfile portable`. Bản auto build trên máy có AVX2 dành cho CPU tương ứng. Chạy EXE Vulkan cần driver/loader Vulkan; không cần SDK hay Ollama/LM Studio trên máy chạy.

```powershell
pnpm test
pnpm build
pnpm desktop:test
pnpm models:smoke
pnpm desktop:build
pnpm bench:realtime -AudioPath path/to/mono-16k-pcm16.wav -Seconds 90
```

Lệnh build hiện tạo bản debug không installer để kiểm thử desktop, stage EXE/DLL tại `out/windows/`. Không coi đây là gói release đã tối ưu kích thước. Chạy EXE từ vị trí này vẫn tìm được `models/` và `runtimes/` ở root; cấu hình folder riêng được giữ trong AppData.

Kiểm thử EXE qua WebView2:

```powershell
./scripts/smoke/launch-gui.ps1
node scripts/smoke/gui-smoke.mjs --verify-restart
./scripts/smoke/stop-gui.ps1
```

Helper chỉ mở cổng CDP trong phiên smoke, chạy EXE với PATH tối thiểu và ghi kết quả vào `.cache/smoke/`. Lệnh kiểm thử model thật có thể mất thời gian nạp model. Test API dùng HTTP giả lập, không cần key thật và không xác nhận quota/chất lượng của tài khoản cloud.

## Dữ liệu

AppData do Tauri xác định, hiện là `%APPDATA%/com.tvnjp.app`:

- `nexq.db`: SQLite, giữ tên để tiếp tục dùng lịch sử cũ.
- `config.json`, `translation-config.json`, `local-models.json`, `rag-config.json`: cấu hình.
- `settings-drafts.json`: cấu hình model/dịch chưa áp dụng; `api-pool.json`: nhãn key và lựa chọn theo vai trò, không chứa secret.
- `context/`, `recordings/`: tài liệu đã nhập và ghi âm/waveform.
- `backups/before-path-migration-*.db`: backup trước khi chuyển tham chiếu đường dẫn cũ.

Key API dùng Windows Credential Manager, mỗi mục Pool có ID riêng. Các namespace cũ như `translation_groq`/`groq` được nhập một lần và giữ nguyên bản gốc. Khi đổi hoặc xóa key trong Pool, backend tạo lại client đang chọn và phát `api_pool_changed` để cả hai cửa sổ cập nhật; thay key của STT cloud sẽ khởi động lại luồng STT đó qua bộ đồng bộ audio. Dữ liệu bí mật không cần đi từ backend lên frontend để chọn provider hoặc nạp catalog.

Migration sao chép file từ AppData NexQ cũ, kiểm tra SHA-256, tránh ghi đè khi trùng tên, rồi cập nhật cột DB. Nếu sao chép lỗi, tham chiếu cũ vẫn giữ để đọc và thử lại lần sau. Không tự xóa thư mục dữ liệu cũ.

## Repository độc lập

Remote `origin` của dự án phải là `https://github.com/QuangNew/Kakehashi.git`. Mở đúng thư mục **Kakehashi** trước khi Commit hoặc Pull. Thư mục thử nghiệm T_VNJP/NexQ cũ có lịch sử khác; không pull/merge toàn bộ NexQ vào Kakehashi. Khi muốn lấy bản sửa upstream, khảo sát diff và đưa từng thay đổi cần thiết vào dự án.
