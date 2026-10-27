# Tiến độ Kakehashi

Cập nhật: **27/10/2026**. Đã thêm **API Pool**, lưu cấu hình/bản nháp, sửa cuộn chuột và thoát thiết lập nhanh, dùng lại model và chuẩn bị nền tùy chọn. Model giữ **Whisper Large v3 Turbo + Hy-MT2 7B Q4**. Kiểm thử: **292 frontend + 267 Rust**; EXE debug và GUI smoke đạt. Nạp model lần đầu **23,8 giây**, dùng lại **0,29–0,32 giây** (chỉ phần chuẩn bị model). Podcast 90 giây: 39/39 bản dịch, không bỏ audio/lỗi dịch; trễ trung vị/p95 **1.780/2.859 ms sau cuối đoạn**, CER Nhật giữ **13,87%**; chưa gồm gom đoạn tối đa 3 giây. Chi tiết ở [API Pool và khởi động](reports/api-pool-startup.vi.md), [báo cáo nâng model trước đó](reports/quality-upgrade.vi.md).

| Hạng mục | Trạng thái |
| --- | --- |
| Phụ đề Việt ↔ Nhật | Hai nguồn mic/system riêng; giữ câu gốc và bản dịch trên overlay, lưu lịch sử |
| Model trực tiếp trong folder | Whisper.cpp trong ứng dụng; runtime llama.cpp được quản lý cho Hy-MT2, copilot GGUF và embedding |
| AI dịch độc lập | Hy-MT2, Groq và Google AI Studio/Gemini có cấu hình riêng; giữ các provider dịch kế thừa |
| AI copilot | Giữ local GGUF, Ollama, LM Studio, Claude CLI/subscription và các API hiện có |
| API Pool | Lưu key một lần, chọn dùng chung hoặc riêng theo STT/dịch/copilot; nhập key cũ, trạng thái xanh/đỏ, prompt mở Pool khi thiếu |
| Cấu hình | Ghi ngay/thử lại khi lỗi, bản nháp model/dịch giữ qua đổi tab và mở lại; áp dụng backend trước khi cập nhật trạng thái đang dùng |
| Vào họp | Giữ trọng số Whisper và server dịch; làm nóng ngắn, chuẩn bị nền tùy chọn, giải phóng bộ nhớ thủ công |
| Tối ưu realtime | Whisper Turbo/Vulkan chọn GPU rời, dùng chung trọng số giữa hai nguồn, chuẩn bị model trước capture, cap 3 giây, queue giới hạn và số đo trên UI |
| Chế độ họp | Online/in-person; Meeting, Interview, Lecture, Webinar và Oral Exam |
| Tài liệu và dữ liệu | Prompt trực tiếp/RAG local, lịch sử, ghi âm/playback; migration đường dẫn sao chép/kiểm tra trước khi cập nhật DB |
| Thu gọn | Bỏ website/deploy, demo giả lập, updater/release upstream và Gemini Context Cache; dùng pnpm |
| Noise Presets / Start on Login | Giữ UI/config gốc; backend xử lý nhiễu/khởi động Windows chưa nối hoàn chỉnh |

## Kiểm chứng trước khi tách repo — mốc 25/10, Whisper Small + Hy-MT2 1.8B

- Đạt **265 test frontend**, **255 test Rust** và **14 test tích hợp Claude session giả lập**. Hai test gọi Claude thật được để `ignored`.
- Build debug Windows đạt; kiểm tra GUI với PATH tối thiểu thấy đủ catalog provider/mode, không có JavaScript exception. DLL ONNX Runtime nạp được và báo phiên bản 1.24.2.
- Đối chiếu dữ liệu trước/sau kiểm thử đạt; dữ liệu sử dụng và bằng chứng cục bộ không đưa vào repository.
- Podcast Nhật 90 giây, tốc độ 1×: **39/39 đoạn đã dịch**, không bỏ audio hay lỗi dịch; trễ dịch trung vị/p95 **505/839 ms tính sau cuối đoạn**. Thời gian này chưa gồm gom đoạn tối đa 3 giây. CER **13,87%** trên mẫu tham chiếu, giữ nguyên so với bản tối ưu trước đó.

Chi tiết phần mềm: [báo cáo repo](reports/repository-report.vi.md). Cách đo, phần cứng và giới hạn: [báo cáo hiệu năng](reports/realtime-performance.vi.md).

## Phần cần kiểm chứng tiếp

- Độ trễ WASAPI → giao diện trong cuộc họp thật, đặc biệt khi hai bên cùng nói.
- Chất lượng dịch Việt bằng tham chiếu của người dịch và kiểm thử API bằng key thật. Các test HTTP giả lập chỉ xác nhận giao thức và xử lý cấu hình.
- Kiểm thử dài hạn các provider được giữ; các lựa chọn STT/cloud khác không được coi là đã benchmark tương đương Whisper Small local.

Build chạy cục bộ ở `out/windows/Kakehashi.exe`, kèm DLL. Repo chưa cung cấp bộ cài hay bản phát hành nhị phân. Hướng dẫn: [sử dụng](guides/usage.vi.md), [phát triển](guides/development.vi.md), [scope](guides/scope.vi.md).
