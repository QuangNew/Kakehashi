# Nâng model và sửa giao diện — 26/10/2026

## Cấu hình hiện tại

- STT: **Whisper Large v3 Turbo**, file 1.624.555.275 byte, revision và SHA-256 ghim trong catalog. Cả microphone và system audio chọn `large-v3-turbo` trong preset Việt–Nhật.
- Dịch local: **Hy-MT2 7B Q4_K_M**, file 4.624.648.896 byte. Mặc định context 2048, 22 lớp GPU; giữ tùy chọn Hy-MT2 1.8B để ưu tiên tốc độ. Model được tải và kiểm tra checksum trước khi cấu hình.
- Whisper chia sẻ trọng số giữa hai nguồn nhưng giữ decoder state/ngôn ngữ riêng; đồng bộ suy luận trên backend dùng chung để tránh nạp hai bản trọng số Turbo vào VRAM.
- Copilot/RAG giữ cấu hình riêng. Groq/Gemini dịch vẫn có provider/model/key độc lập.

Nguồn model: [Whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp), [Tencent Hy-MT2-7B-GGUF](https://huggingface.co/tencent/Hy-MT2-7B-GGUF). Hash, đường dẫn và phiên bản tải ở [models.lock.json](../../scripts/models.lock.json).

## Dịch theo ý và ngữ cảnh

Hy-MT2, Groq và Gemini nhận hướng dẫn dịch theo nghĩa, diễn đạt tự nhiên theo trật tự ngôn ngữ đích, giữ phủ định/mức độ chắc chắn/tên/số. Khi dịch sang Nhật, mặc định dùng cách nói lịch sự trung tính です/ます, không đoán giới tính hoặc tự thêm đại từ.

Mỗi lượt dịch trực tiếp nhận tối đa hai đoạn đã dịch trước đó thuộc cùng cặp ngôn ngữ làm tham chiếu. Không gửi lại toàn bộ lịch sử; giới hạn 240 ký tự cho mỗi phần nguồn/đích. Phần hiện tại được đánh dấu riêng, yêu cầu không lặp lại bản dịch của ngữ cảnh. Sửa cùng một segment không dùng phiên bản cũ của chính nó làm tham chiếu. Ngữ cảnh bị xóa khi kết thúc/đổi phiên, không dùng chung giữa hai chiều.

Không tăng thời gian gom audio để chờ câu dài hơn. Vì thế, đoạn cắt giữa câu vẫn có thể thiếu thông tin chưa được nói; model lớn và ngữ cảnh không bảo đảm giải quyết mọi trường hợp. Đã thử các câu Việt về xin thêm thời gian, cập nhật tiến độ, hoãn lịch thay vì hủy, và chưa thể cam kết ngày giao. Đây là smoke với model thật, chưa phải đánh giá dịch độc lập của người bản ngữ.

## Kết quả podcast

Máy: i7-13700HX, RAM 16 GB, RTX 4050 Laptop 6 GB, Windows. Mẫu 90 giây ở tốc độ 1×, khoảng 00:30–02:00 của [podcast Nhật](https://www.youtube.com/watch?v=SmxSCZzV7j0), cùng WAV/phụ đề tham chiếu với báo cáo trước. System JA, một context mic VI im lặng; Whisper Turbo + Hy-MT2 7B có ngữ cảnh.

| Chỉ số | 22 lớp GPU — mặc định | 24 lớp GPU — tùy chỉnh |
| --- | ---: | ---: |
| Đoạn / bản dịch | 39 / 39 | 39 / 39 |
| Audio bị bỏ / lỗi dịch | 0 / 0 | 0 / 0 |
| Trễ STT trung vị / p95 | 296 / 405 ms | 245 / 477 ms |
| Trễ dịch trung vị / p95 | 1.920 / 3.083 ms | 1.656 / 2.706 ms |
| Suy luận dịch trung vị / p95 | 1.328 / 2.155 ms | 1.079 / 1.855 ms |
| Chờ dịch trung vị / lớn nhất | 0 / 976 ms | 0 / 649 ms |
| Đỉnh tổng VRAM máy | 5.544 / 6.141 MiB | 5.808 / 6.141 MiB |
| CER trên 375 ký tự JA chuẩn hóa | 13,87% | 13,87% |

**Trễ tính sau cuối đoạn audio, chưa gồm gom đoạn tối đa 3 giây.** 22 lớp dành thêm VRAM cho giao diện/ứng dụng cuộc gọi, giữ cùng trọng số model. 24 lớp nhanh hơn trong phép đo này nhưng gần giới hạn VRAM hơn. Tải máy giữa các lượt khác nhau, không coi chênh lệch này là bảo đảm trên mọi máy.

Thử 16 lớp GPU không theo kịp: queue tăng và chỉ 37/39 bản dịch hoàn thành trước thời hạn của harness. Vì thế không dùng cấu hình 16 lớp làm mặc định. Mốc 1.8B trước đó có độ trễ thấp hơn; tăng model ưu tiên khả năng dịch, không tuyên bố nhanh hơn bản Small/1.8B.

CER không giảm so với mốc trước trên đoạn mẫu này. Chưa có chấm điểm bản dịch Việt bằng tham chiếu của người dịch, thử dài hạn hoặc cuộc gọi thật hai bên cùng nói. Harness dùng cùng provider/native feature nhưng không đo toàn tuyến WASAPI → giao diện.

## Lỗi tải model và giao diện đã sửa

Backend giữ snapshot tiến trình có revision cho từng lượt tải. Giao diện đăng ký sự kiện trước khi đọc snapshot, bỏ snapshot cũ nếu đã nhận sự kiện mới; store và listener cấp ứng dụng giữ trạng thái khi chuyển tab. Khóa nút ngay khi bấm để tránh hai yêu cầu liên tiếp. Lỗi tải giữ thông báo và nút Resume thay vì tự biến mất sau ba giây.

Lượt tải có trạng thái kết thúc thật, không suy đoán “đang tải” từ sự tồn tại của file tạm. Retry dùng HTTP Range/If-Range, kiểm tra Content-Range, xử lý trường hợp server bỏ qua Range, giữ phần tải dở và chỉ cài sau khi đủ độ dài/checksum. Hủy lượt tải chờ hoàn tất các lần ghi file trước khi cho phép tải lại.

Modal Start Meeting giới hạn theo chiều cao cửa sổ, nội dung và danh sách scenario có vùng cuộn riêng; bỏ dropdown absolute bị cắt bởi container. Đã kiểm tra bằng wheel ở viewport 960×540, chọn được Oral Exam và cuộn tới nút Start. Nhãn Whisper “batch only” cũ đã thay bằng nhận dạng trực tiếp local/GPU.

## Kiểm chứng

- 270 test frontend, 264 test Rust đạt; TypeScript/Vite và build debug Windows đạt.
- Regression kiểm tra remount tab, snapshot đến muộn, double click, khôi phục sau lỗi và gỡ listener đúng cách.
- HTTP fixture kiểm tra đứt body/tải tiếp, server bỏ qua Range, checksum sai và hủy trong lúc body đứng; file chưa hợp lệ không được cài.
- GUI thực tế: scenario cuộn/chọn được; sự kiện tải giả lập đi qua Tauri vẫn cập nhật khi tab đóng và hiện đúng khi quay lại; 0 JavaScript exception. Không tải model lần hai hay tạo cuộc họp trong phép thử UI này.
- Model, dữ liệu sử dụng, audio/phụ đề nguồn, log thô và bằng chứng smoke được giữ cục bộ ngoài Git.

Chạy và thiết lập: [hướng dẫn model](../guides/local-models.vi.md). Tiến độ chung: [PROGRESS](../PROGRESS.vi.md).
