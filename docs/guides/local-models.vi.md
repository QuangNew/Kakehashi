# Model local trong folder

Kakehashi dùng Whisper trong ứng dụng và tự quản lý `llama-server.exe` cho dịch, copilot và embedding. Cần cả trọng số lẫn runtime phù hợp. Có thể chọn API riêng cho dịch hoặc copilot tại Settings; xem [hướng dẫn sử dụng](usage.vi.md).

## Chuẩn bị

```powershell
pnpm models:setup
pnpm models:setup -WithCopilot -WithEmbeddings
```

Lệnh đầu tải Whisper Large v3 Turbo, Hy-MT2 7B Q4 và runtime; hai cờ bổ sung tải Qwen copilot/embedding. Script ghim revision/SHA-256 tại [models.lock.json](../../scripts/models.lock.json), hỗ trợ tải tiếp phần dở và kiểm tra checksum.

Có thể truyền `-ModelsRoot D:\AI\models -RuntimeRoot D:\AI\llama`, rồi chọn lại đường dẫn trong Settings → Local Models.

```text
workspace/
  out/windows/Kakehashi.exe + DLL
  runtimes/llama/llama-server.exe + DLL
  models/
    whisper_cpp/ggml-large-v3-turbo.bin
    translation/Hy-MT2-7B-Q4_K_M.gguf
    copilot/Qwen3-1.7B-Q4_K_M.gguf
    embeddings/Qwen3-Embedding-0.6B-Q8_0.gguf
```

Giữ các DLL cạnh EXE. Model/runtime vẫn ở root hoặc folder riêng đã cấu hình. EXE trong `out/windows/` tìm model/runtime qua thư mục cha khi chưa có cấu hình riêng.

## Áp dụng trong ứng dụng

1. Mở Settings → **Local Models · VI/JA**, kiểm tra folder model và đường dẫn runtime.
2. **Áp dụng chế độ Việt–Nhật** chọn Whisper Large v3 Turbo cho hai nguồn, mic Việt/system Nhật, Hy-MT2 và phụ đề song ngữ.
3. **Dùng copilot local** chọn Qwen cho AI2; AI1 vẫn độc lập. Có thể dùng API thay cho một trong hai vai trò.
4. **Dùng RAG local (1024d)** chọn embedding local. Thêm tài liệu, rồi dựng lại chỉ mục trong Context Strategy.
5. Kiểm tra thiết bị phát âm thanh trong Audio & Devices trước khi bắt đầu họp.

Nếu chưa có chỉ mục hoặc RAG không tìm được kết quả, AI action có bật tài liệu dùng phần nội dung đính kèm vừa với context. Tài liệu dài nên được lập chỉ mục. Đổi embedding model/dimension làm mất tính tương thích của chỉ mục cũ và cần dựng lại.

## Cấu hình cho i7-13700HX / RAM 16 GB / RTX 4050 6 GB

| Vai trò | Model | Trọng số | Mặc định |
| --- | --- | --- | --- |
| STT | Whisper Large v3 Turbo đa ngôn ngữ | 1,625 GB | GPU rời qua Vulkan; hai nguồn dùng chung trọng số, state riêng |
| Dịch | Hy-MT2 7B Q4_K_M | 4,625 GB | 22 lớp GPU, context 2048; có thể chỉnh theo VRAM |
| Copilot | Qwen3 1.7B Q4_K_M | 1,107 GB | CPU, context 4096 |
| RAG | Qwen3 Embedding 0.6B Q8_0 | 0,639 GB | CPU, context 2048, vector 1024 chiều |

Dung lượng file khác RAM/VRAM khi chạy. Runtime ghim llama.cpp b11669, Windows x64 Vulkan. Whisper và dịch dùng GPU; model Whisper được dùng chung trọng số giữa mic/system để giảm VRAM. Hai chiều dịch nhận tối đa hai đoạn trước cùng ngôn ngữ làm tham chiếu, không dịch lặp phần ngữ cảnh. Copilot/embedding dùng CPU để giảm tranh VRAM. Whisper chọn GPU rời theo loại thiết bị, tránh chọn nhầm Intel UHD khi GPU này có chỉ số 0. Có thể điều chỉnh số lớp GPU và context của các model GGUF ở phần tài nguyên model. Số đo 22/24 lớp cùng giới hạn: [báo cáo nâng model](../reports/quality-upgrade.vi.md).

Đợi bước chuẩn bị model hoàn tất rồi bắt đầu nói/phát video. Trong cuộc họp, thanh trạng thái hiện **Whisper GPU**; rê chuột để xem tên GPU. Số trễ STT tính từ cuối đoạn audio. Phụ đề chốt theo khoảng lặng hoặc độ dài tối đa 3 giây; nếu queue quá tải gây bỏ audio sẽ hiện cảnh báo kèm số giây. Turbo lớn hơn Small, cần benchmark trên máy chạy; CPU dự phòng không đảm bảo realtime.

Server chỉ bind loopback ở 18181/18182/18183. Ứng dụng dừng server do nó tạo khi thoát; không tự dừng server bên ngoài. Cổng bị chiếm được báo lỗi. Với copilot context nhỏ, giảm transcript window trong AI Actions; tổng kết họp dài có thể cần API hoặc context lớn hơn.

## Kiểm thử

```powershell
pnpm models:smoke
```

Smoke kiểm tra dịch hai chiều, copilot, embedding và nạp Whisper thật. Đây không phải phép đo nhận dạng/độ trễ của cuộc họp audio thực tế. Xem [tiến độ](../PROGRESS.vi.md) và báo cáo kiểm thử hiện tại trước khi kết luận bản đang dùng đã được xác minh.

Đo realtime và đối chiếu phụ đề: [báo cáo hiệu năng](../reports/realtime-performance.vi.md). Có lệnh `pnpm bench:realtime -AudioPath <WAV-16k-mono> -Seconds 90`; harness dùng model local và không sửa lịch sử hay thiết lập người dùng.

Config/model paths lưu tại AppData `com.tvnjp.app`; xem [hướng dẫn dữ liệu](development.vi.md). Key API dùng credential manager.

NexQ dùng MIT, giữ [LICENSE](../../LICENSE). Runtime và trọng số có giấy phép riêng: [llama.cpp](https://github.com/ggml-org/llama.cpp), [Whisper](https://huggingface.co/ggerganov/whisper.cpp), [Hy-MT2](https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF), [Qwen3](https://huggingface.co/unsloth/Qwen3-1.7B-GGUF), [Qwen3 Embedding](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF). Model dịch 7B: [Tencent Hy-MT2-7B-GGUF](https://huggingface.co/tencent/Hy-MT2-7B-GGUF).

## Tải model và phục hồi

Tiến trình STT thuộc backend. Chuyển tab hoặc đóng Settings vẫn giữ lượt tải; mở lại khôi phục snapshot và nhận sự kiện mới. Khi mất kết nối, downloader tự retry có giới hạn bằng HTTP Range; hết retry hiển thị lỗi và nút Resume. File dở được giữ, chỉ cài model sau khi kiểm tra độ dài/checksum. Large v3 Turbo dùng revision và SHA-256 ghim trong catalog.

Nếu cần giảm độ trễ dịch, có thể chọn model 1.8B trong Local Models và tải bằng `pnpm models:setup -TranslationProfile fast`. Không tự chuyển model khi quá tải; theo dõi queue và số đo. Groq/Gemini cho AI dịch vẫn dùng cấu hình độc lập với copilot.
