# Kakehashi: cấu trúc và khả năng hiện tại

Ngày 25/10/2026. Kakehashi là ứng dụng Windows phát triển từ NexQ, dùng Tauri 2, React/TypeScript, Rust và SQLite. Phạm vi đầu tiên là phụ đề Việt–Nhật và trợ lý cuộc họp; chưa có TTS hay virtual microphone.

## Những gì có trong repository

| Đường dẫn | Nội dung |
| --- | --- |
| `src/` | Launcher, overlay, settings, transcript, lịch sử, AI actions, hook và store |
| `src-tauri/src/audio/` | Thu mic/system audio, chọn thiết bị và ghi âm |
| `src-tauri/src/stt/` | Whisper.cpp, chia đoạn/queue, các provider STT kế thừa và quản lý model |
| `src-tauri/src/translation/` | Hy-MT2, API dịch độc lập, các provider dịch và router |
| `src-tauri/src/llm/` | Copilot local/API/Claude CLI và quản lý session |
| `src-tauri/src/context/`, `src-tauri/src/rag/`, `src-tauri/src/intelligence/` | Tài liệu, embedding/truy xuất và phân tích cuộc họp |
| `src-tauri/src/commands/` | IPC nối giao diện với backend, vòng đời cuộc họp và cấu hình |
| `scripts/` | Thiết lập model/runtime, build Windows, smoke và tổng hợp benchmark |
| `src-tauri/examples/`, `src-tauri/tests/` | Harness audio realtime và kiểm thử tích hợp CLI |
| `docs/guides/` | Hướng dẫn hiện tại; báo cáo ở `docs/reports/` |
| `.github/` | CI pnpm, mẫu issue và pull request |

Model, runtime, công cụ native, dependencies cài đặt, EXE/DLL, cache và dữ liệu người dùng được tạo cục bộ, không có trong Git.

## Luồng chức năng

Online thu mic và system audio riêng; mặc định mic Việt → Nhật, system Nhật → Việt. Whisper Small local/Vulkan chốt đoạn theo khoảng lặng hoặc cap 3 giây, chuyển sang AI dịch độc lập rồi hiện phụ đề song ngữ. Copilot đọc transcript và tài liệu theo cấu hình AI Actions. In-person dùng một mic chung với provider Room/Them.

| Vai trò | Provider được giữ |
| --- | --- |
| STT | Whisper.cpp, Windows Speech, Web Speech, Deepgram, Whisper API, Azure Speech, Groq Whisper, Sherpa-ONNX, ORT Streaming, Parakeet |
| Dịch | Hy-MT2, Groq, Gemini, Microsoft, Google Cloud Translation, DeepL, OPUS-MT, LLM dùng chung copilot |
| Copilot | Local GGUF, Ollama, LM Studio, Claude CLI/subscription, OpenAI, Anthropic, Groq, Gemini, OpenRouter, Custom API |

Groq/Gemini dịch có key/model riêng với copilot. Chỉ lựa chọn dịch dùng chung LLM mới lấy cấu hình copilot. Các provider khác nhau có yêu cầu tài khoản, model, ngôn ngữ và nền tảng riêng.

Giữ Meeting/Interview/Lecture/Webinar/Oral Exam, prompt/export tương ứng, RAG local, lịch sử, ghi âm/playback, overlay, stealth và hotkeys. Noise Presets/Start on Login còn ở trạng thái UI/config gốc, chưa nối hoàn chỉnh với backend.

## Thay đổi Kakehashi

- Đổi tên hiển thị/icon/EXE; giữ app ID `com.tvnjp.app` và schema để tiếp tục đọc dữ liệu cũ.
- Thêm model trong folder và runtime có quản lý tiến trình; mặc định GPU cho Whisper/dịch, CPU cho copilot/embedding để giảm tranh VRAM.
- Tách provider/key/model AI dịch khỏi copilot, thêm Groq và Google AI Studio/Gemini cho dịch.
- Sửa build Vulkan, chọn GPU rời, liên kết runtime C++ GNU, chuẩn bị model trước capture và pipeline audio một lượt; thêm số đo/queue cảnh báo.
- Tích hợp chọn lọc các bản sửa upstream cho audio settings, Custom API, reset cuộc họp, tài liệu fallback và reconnect.
- Chuyển sang pnpm; bỏ website/demo/updater upstream và Gemini Context Cache; tách vòng đời lưu cuộc họp/ghi âm khỏi updater.
- Migration tài liệu/ghi âm sao chép và kiểm tra checksum trước khi cập nhật tham chiếu, giữ dữ liệu gốc.

Trạng thái kiểm thử và phần chưa hoàn tất ở [PROGRESS](../PROGRESS.vi.md). Hiệu năng đo được cùng giới hạn ở [báo cáo realtime](realtime-performance.vi.md). Bộ [hướng dẫn upstream](upstream-user-guide/README.md) chỉ dùng tham chiếu.
