# Phạm vi hiện tại

Điều chỉnh ngày 25/10/2026: **giữ lại G4–G5, G6, G7, G9–G10**. Chỉ thu gọn G1–G3 và G8; giữ model/runtime local và migration dữ liệu. Trạng thái kiểm thử ở [PROGRESS](../PROGRESS.vi.md); cấu trúc hiện tại ở [báo cáo repo](../reports/repository-report.vi.md).

| Vai trò | Lựa chọn |
| --- | --- |
| STT hai nguồn | Whisper.cpp Small/Vulkan là mặc định; giữ Windows Speech, Web Speech, Deepgram, Whisper API, Azure Speech, Groq Whisper, Sherpa-ONNX, ORT Streaming và Parakeet |
| AI 1 — dịch | Hy-MT2 / Groq / Google AI Studio–Gemini với cấu hình riêng; giữ Microsoft Translator, Google Cloud Translation, DeepL, OPUS-MT và tùy chọn dùng chung active copilot |
| AI 2 — copilot | Local GGUF / Ollama / LM Studio / Claude CLI–subscription / Groq / Gemini / OpenAI / Anthropic / OpenRouter / Custom API |
| Tài liệu | Đưa trực tiếp vào prompt hoặc truy xuất RAG bằng embedding local |
| Cuộc họp | Online hoặc in-person với mic chung; Meeting/Interview/Lecture/Webinar/Oral Exam và prompt tùy chỉnh |
| Được giữ | Overlay, phụ đề hai chiều, lịch sử, ghi âm/playback, stealth, hotkeys, AI actions, Noise Presets và Start on Login |

Đã loại khỏi thiết kế: website/deploy NexQ, demo giả lập, updater/release upstream và Gemini Context Cache. Bộ hướng dẫn upstream vẫn được lưu làm tham chiếu.

Giữ định dạng metadata của lịch sử và app ID `com.tvnjp.app`; các provider được khôi phục có thể chọn và lưu lại. Không xóa model, credential hay lịch sử. Migration đường dẫn chỉ cập nhật file được DB tham chiếu sau khi sao chép/kiểm tra; file gốc ở đường dẫn cũ vẫn còn.

Groq/Gemini dành cho AI dịch vẫn có model/key riêng với copilot. Chỉ tùy chọn **LLM dùng chung provider/model copilot** mới dùng cấu hình copilot. Noise Presets và Start on Login được giữ ở trạng thái chức năng gốc; các preference này chưa được nối hoàn chỉnh với xử lý âm thanh/khởi động Windows.

`models/`, `runtimes/`, `src-tauri/target/`, `node_modules/` và `.native-tools/` là tài nguyên cục bộ, không đưa vào Git. Repository Kakehashi public chứa mã nguồn và hướng dẫn; dữ liệu, model và bản build phải được chuẩn bị trên máy chạy.
