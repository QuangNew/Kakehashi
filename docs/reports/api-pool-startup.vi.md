# API Pool, cấu hình và thời gian vào họp — 27/10/2026

## Thay đổi

- API Pool có nhiều key có tên gợi nhớ theo nhà cung cấp. STT, AI dịch và copilot dùng key mặc định hoặc chọn key riêng. Secret chỉ lưu trong Windows Credential Manager; giao diện chọn key không nhận lại giá trị bí mật.
- Nhập các credential cũ một lần, giữ lại bản gốc và giữ riêng các tài khoản dịch/copilot trước đây. Xóa key đang được chọn không tự chuyển sang tài khoản khác.
- Nhãn xanh thể hiện có key đã lưu, nhãn đỏ yêu cầu mở Pool; không dùng màu xanh để khẳng định quota hay chất lượng. Kiểm tra catalog Groq/Gemini/OpenAI/Anthropic/OpenRouter không đổi provider đang chạy.
- Backend tạo lại client khi thay key; thông báo cả hai cửa sổ qua sự kiện. Giao diện chỉ ghi nhận provider/model sau khi backend chấp nhận. Catalog copilot loại các họ model audio/embedding đã biết. Model Whisper thuộc API nhận dạng tiếng nói ([tài liệu Groq](https://console.groq.com/docs/speech-to-text)), không dùng làm model chat.
- Cấu hình được ghi theo hàng đợi, có lưu ngay/thử lại và thông báo lỗi. Bản nháp file model/AI dịch tồn tại qua đổi tab và mở lại ứng dụng; backend chỉ đổi khi áp dụng.
- Wizard có nút đóng ở mọi bước, Esc và thiết lập sau; lỗi ghi đĩa được báo và không khóa đường thoát. Modal, Settings, provider picker và API Pool có vùng cuộn giới hạn theo chiều cao cửa sổ.
- Whisper giữ trọng số dùng chung giữa các cuộc họp, giữ decoder riêng cho hai nguồn. Khi đổi sang model khác, bộ nhớ model cũ không còn dùng được giải phóng. Có nút giải phóng thủ công khi không họp.
- Hy-MT2 chỉ làm nóng bằng hai câu ngắn với giới hạn 8 token, nhớ PID server đã chuẩn bị. Không sinh hai đoạn dịch dài trước mỗi lần vào họp. Tự chuẩn bị nền là lựa chọn người dùng; mặc định tắt.
- Runtime dịch dùng 8 luồng CPU, batch 256 / ubatch 128. Debounce câu đã chốt giảm từ 200 xuống 40 ms; giữ ngữ cảnh hai lượt, kiểm soát phiên và chống ghi đè bản sửa.

## Nạp model thật

i7-13700HX, RAM 16 GB, RTX 4050 6 GB, Vulkan; Whisper Large v3 Turbo và Hy-MT2 7B Q4_K_M, context 2048, 22 lớp GPU. Đo trong một tiến trình mới, lần đầu và hai lần tạo lại decoder khi vẫn giữ trọng số/server:

| Lần chuẩn bị | Dịch | Hai nguồn Whisper | Tổng |
| --- | ---: | ---: | ---: |
| Đầu tiên | 17.824 ms | 5.957 ms | 23.783 ms |
| Dùng lại lần 1 | 30 ms | 288 ms | 319 ms |
| Dùng lại lần 2 | 0 ms | 292 ms | 292 ms |

VRAM đỉnh lấy mẫu: 5.344 / 6.141 MiB. Đây là thời gian chuẩn bị model, chưa gồm tạo cửa sổ/cuộc họp, mở thiết bị và WASAPI. Lần đầu tính theo tiến trình mới, không xóa cache ổ đĩa của hệ điều hành. Số liệu có thể khác khi ứng dụng khác sử dụng GPU.

Log bản trước cho thấy riêng phần dịch cần khoảng 41,8 giây, gồm nạp server và làm nóng bằng hai đoạn dài. Lựa chọn chuẩn bị nền chuyển việc nạp sang trước lúc bấm họp; không làm mất chi phí nạp vật lý. Không dùng model nhỏ hơn hoặc giảm chất lượng giải mã để đạt số đo trên.

## Kiểm thử và giới hạn

- 292 test frontend, 267 test Rust; TypeScript/Vite và EXE Windows debug build thành công.
- Smoke trên EXE: lăn chuột chọn Oral Exam ở 960×540, cuộn Settings/API Pool, prompt thiếu key, chỉ có một nơi nhập key, đóng/Esc wizard, bản nháp tồn tại sau đổi tab/tải lại và backend không đổi theo bản nháp. Không có JavaScript exception.
- Kiểm tra native IPC chuẩn bị model trên GUI thật: trạng thái `ready` và số model giữ trong bộ nhớ khớp backend; lần dùng lại không tải lại trọng số; nút giải phóng đưa trạng thái về `idle`, số model giữ về 0. Việc khôi phục JSON đã đổi thứ tự thuộc tính không tạo cảnh báo bản nháp giả.
- Key thật không được gửi lên API để kiểm thử chất lượng; các test giao thức dùng HTTP giả lập. Trạng thái có key không xác nhận quota.
- Không tuyên bố cải thiện tổng quát chất lượng dịch khi chưa có tham chiếu tiếng Việt do người dịch đánh giá.

## Realtime trên podcast mẫu

Cùng đoạn 00:30–02:00 trong [video người dùng cung cấp](https://www.youtube.com/watch?v=SmxSCZzV7j0), phát PCM16 mono 16 kHz ở tốc độ 1×, nguồn Nhật nói và nguồn Việt im lặng. Cùng model, ngôn ngữ, cap 3 giây, 22 lớp GPU, prompt/ngữ cảnh và tham chiếu Nhật theo caption đã căn thời gian; transcript người dùng cung cấp dùng để đọc đối chiếu nội dung.

| Chỉ số | Bản 26/10 | Bản 27/10 |
| --- | ---: | ---: |
| Đoạn nhận dạng / đã dịch | 39 / 39 | 39 / 39 |
| Âm thanh bị bỏ / lỗi dịch | 0 / 0 | 0 / 0 |
| STT sau cuối đoạn, trung vị / p95 | 296 / 405 ms | 289 / 428 ms |
| Bản dịch sau cuối đoạn, trung vị / p95 | 1.920 / 3.083 ms | 1.780 / 2.859 ms |
| Thời gian suy luận dịch, trung vị / p95 | 1.328 / 2.155 ms | 1.296 / 2.133 ms |
| CER Nhật trên tham chiếu 375 ký tự | 13,87% | 13,87% |
| VRAM đỉnh lấy mẫu | 5.544 MiB | 5.459 MiB |

Trung vị và p95 độ trễ dịch giảm khoảng 7,3%. STT p95 tăng 23 ms trên lần đo này; không xem chênh lệch nhỏ này là cải thiện hay suy giảm tổng quát. Thời gian sau cuối đoạn **chưa gồm gom âm thanh tối đa 3 giây**. Không đo WASAPI → giao diện, không mô phỏng hai người cùng nói, không có chấm điểm bản dịch tiếng Việt bằng tham chiếu người dịch. Thay đổi này giữ nguyên model và thiết lập suy luận nội dung; CER bằng nhau trên một mẫu không đảm bảo mọi cuộc họp sẽ có độ chính xác bằng nhau.

## Repository

Repository phát triển độc lập: `https://github.com/QuangNew/Kakehashi.git`, thư mục `Kakehashi`. Các conflict người dùng gặp nằm ở thư mục T_VNJP đang merge NexQ; Git ở đó vẫn trỏ `naxhq/NexQ`. Đã sao lưu worktree/index/lịch sử trước khi xử lý, giữ nguyên model và dữ liệu. Quyền Windows chặn đổi metadata `.git` của thư mục cũ, nên cần mở workspace Kakehashi để Commit/Pull đúng repo. Không thay quyền ACL hoặc ép sửa metadata được bảo vệ.

Model, log, caption/audio mẫu, dữ liệu cá nhân và bản build ở ngoài Git. Xem [hướng dẫn sử dụng](../guides/usage.vi.md) để bật chuẩn bị nền và chọn API Pool.
