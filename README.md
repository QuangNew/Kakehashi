# Kakehashi · かけはし

Ứng dụng Windows hiển thị phụ đề song ngữ Việt–Nhật và trợ lý cuộc họp, phát triển từ [NexQ](https://github.com/naxhq/NexQ), giấy phép MIT. Kakehashi có repository và lịch sử Git riêng; thông tin nguồn gốc ở [NOTICE.md](NOTICE.md).

- Microphone và system audio có ngôn ngữ riêng; mặc định Whisper Large v3 Turbo local dùng GPU Vulkan. Có thêm các provider STT cloud, browser, Windows Speech và ONNX/Sherpa/Parakeet.
- AI dịch: Hy-MT2 7B trong folder, Groq API hoặc Google AI Studio / Gemini API độc lập; giữ Microsoft/Google Cloud Translation, DeepL, OPUS-MT và tùy chọn dùng chung copilot.
- AI copilot chọn provider/model/key riêng; hỗ trợ local GGUF, Ollama, LM Studio, Claude subscription/CLI, Groq, Gemini, OpenAI, Anthropic, OpenRouter và Custom API.
- Overlay song ngữ, gợi ý trả lời, Meeting/Interview/Lecture/Webinar/Oral Exam, chế độ online/in-person, prompt tùy chỉnh, tài liệu/RAG local, lịch sử, ghi âm và nghe lại, stealth/hotkeys.

**Tiến độ triển khai:** [docs/PROGRESS.vi.md](docs/PROGRESS.vi.md).

## Chạy và phát triển

Windows, PowerShell 7, Node.js 24, pnpm theo `packageManager` và Rust GNU. Script desktop chuẩn bị CMake/Ninja/libclang portable khi cần; xem [hướng dẫn phát triển](docs/guides/development.vi.md).

```powershell
git clone https://github.com/QuangNew/Kakehashi.git
cd Kakehashi
pnpm install --frozen-lockfile
pnpm models:setup
pnpm desktop:build
./out/windows/Kakehashi.exe
```

Đầu ra EXE và DLL nằm trong `out/windows/`. Trọng số nằm tại `models/`, llama.cpp tại `runtimes/llama/`. Ứng dụng quản lý runtime local, có thể chọn folder/file khác trong Settings → Local Models.

Repo chứa mã nguồn và script thiết lập; model, runtime, bản build, key API và dữ liệu cuộc họp được lưu ngoài Git. Lệnh build hiện tạo bản debug Windows để kiểm thử, chưa phải bộ cài release.

```powershell
pnpm desktop:dev    # Tauri + Vite
pnpm test           # Test giao diện và store
pnpm build          # TypeScript + Vite
pnpm desktop:test   # Kiểm tra Rust và unit test
pnpm models:smoke   # Kiểm thử các model thật đang có
```

## Hướng dẫn

- [Thiết lập model local](docs/guides/local-models.vi.md)
- [Sử dụng và cấu hình hai AI](docs/guides/usage.vi.md)
- [Phạm vi hiện tại](docs/guides/scope.vi.md)
- [Phát triển, kiểm thử và dữ liệu](docs/guides/development.vi.md)
- [Cấu trúc và khả năng hiện tại](docs/reports/repository-report.vi.md)
- [Hiệu năng realtime và giới hạn phép đo](docs/reports/realtime-performance.vi.md)
- [Nâng model Turbo/7B và sửa giao diện](docs/reports/quality-upgrade.vi.md)
- [Bản sửa upstream đã chọn](docs/reports/upstream-sync.vi.md), [hướng dẫn NexQ tham chiếu](docs/reports/upstream-user-guide/README.md)

Dữ liệu ứng dụng dùng `%APPDATA%/com.tvnjp.app`. File tài liệu/ghi âm có tham chiếu từ đường dẫn NexQ cũ được sao chép và kiểm tra trước khi cập nhật DB; bản gốc và backup DB được giữ. Key API lưu qua credential manager của Windows.

Hiện ưu tiên phụ đề song ngữ; chưa có TTS hay virtual microphone. Xem tiến độ để biết kiểm thử nào đã hoàn tất, cùng những giới hạn của bản đang phát triển.

## Nguồn gốc

NexQ: Tauri 2, React, Rust, SQLite. Giữ [LICENSE](LICENSE) và [CHANGELOG](CHANGELOG.md) của nguồn gốc. Model/runtime có giấy phép riêng tại nguồn tải trong [models.lock.json](scripts/models.lock.json).
