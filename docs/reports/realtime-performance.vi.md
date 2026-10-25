# Độ trễ và chất lượng phụ đề Kakehashi

Đo ngày 25/10/2026 trên i7-13700HX, RAM 16 GB, RTX 4050 Laptop 6 GB, Windows. Nguồn: [podcast Nhật](https://www.youtube.com/watch?v=SmxSCZzV7j0), đoạn khoảng 00:30–02:00. WAV 90 giây, mono PCM16 16 kHz; Whisper Small và Hy-MT2 Q4_K_M không thay trọng số để lấy tốc độ.

## Những thay đổi đã làm

- Bật Whisper Vulkan và chọn GPU rời theo loại thiết bị. Trên máy đo, Intel UHD là Vulkan0 và RTX 4050 là Vulkan1.
- Sửa lỗi runtime C++/Rust GNU trong `std::call_once` khi suy luận Vulkan: liên kết static runtime C++ và giữ đối tượng once/TLS cần thiết.
- Xử lý mỗi đoạn audio một lần; chốt theo khoảng lặng thật hoặc cap 3 giây, gửi final đi dịch ngay. Hai nguồn dùng chung đồng hồ capture.
- Chuẩn bị pipeline encoder/decoder Whisper và cả hai chiều dịch trước capture; không tính lần biên dịch shader đầu vào thời gian thu âm.
- Giới hạn queue, hiển thị GPU/CPU và độ trễ, cảnh báo rõ nếu quá tải làm bỏ audio.

Bản CPU ban đầu mất 47,5–53,7 giây suy luận cho 3 giây audio trên máy đo. Suy luận GPU sau khởi tạo đạt khoảng 111–113 ms cho 3 giây audio trong probe. Đây là thời gian suy luận, không phải độ trễ phụ đề hoàn chỉnh.

## Kết quả bản hiện tại

Chạy audio tốc độ 1× với system JA và một context mic VI im lặng; giữ toàn bộ provider/chế độ của scope hiện tại.

| Chỉ số | Kết quả |
| --- | ---: |
| Final / bản dịch | 39 / 39 |
| Audio bị bỏ / lỗi dịch | 0 ms / 0 |
| Trễ STT sau cuối đoạn: trung vị / p95 / lớn nhất | 161 / 357 / 482 ms |
| Trễ dịch sau cuối đoạn: trung vị / p95 / lớn nhất | 505 / 839 / 885 ms |
| Suy luận dịch: trung vị / p95 / lớn nhất | 121 / 352 / 379 ms |
| Chờ dịch: trung vị / lớn nhất | 0 / 0 ms |
| RTF Whisper | 0,0717 |
| CER so với phụ đề JA của kênh | 13,87% — 52 lỗi / 375 ký tự chuẩn hóa |
| Đỉnh tổng VRAM máy, 139 mẫu 1 Hz | 2.838 / 6.141 MiB |

RTF nhỏ hơn 1 là xử lý nhanh hơn tốc độ audio. Các độ trễ **tính từ cuối đoạn audio, chưa gồm gom đoạn tối đa 3 giây**. VRAM gồm cả tiến trình desktop khác.

Trước khi giữ lại các provider/chế độ kế thừa, cùng mẫu đạt trễ dịch trung vị/p95 636/1.780 ms và CER 13,87%. Tải máy giữa các lần đo khác nhau; không quy kết việc thêm provider làm giảm độ trễ.

## Đối chiếu nhận dạng

CER chuẩn hóa NFKC, bỏ dấu câu/khoảng trắng:

| Cách nhận dạng | CER trên mẫu 90 giây |
| --- | ---: |
| Ghép đầu ra từng 1 giây, tham chiếu fast-pass cũ không rớt queue | 51,73% |
| Cửa sổ độc lập 3 giây | 14,93% |
| Cửa sổ độc lập 30 giây | 16,00% |
| Luồng mới, khoảng lặng / cap 3 giây | 13,87% |

Mẫu chỉ có 375 ký tự tham chiếu. Cách viết kanji/kana và cue giao biên ảnh hưởng CER; kết quả không chứng nhận độ chính xác chung. Chưa có bản dịch Việt được người dịch kiểm chứng để chấm chất lượng dịch.

## Phạm vi và tái lập

Harness dùng cùng provider STT, Hy-MT2 và feature native với desktop, cấp audio tốc độ 1×, hai context Whisper, debounce dịch 200 ms và queue dịch tuần tự. Chưa phải phép thử WASAPI → giao diện hay cuộc gọi thật có hai bên cùng nói; chưa gọi cloud API bằng key thật.

```powershell
pnpm models:setup
pnpm bench:realtime -AudioPath path/to/mono-16k-pcm16.wav -Seconds 90
```

Helper không tự tải video. Log JSON mặc định không in transcript. `scripts/smoke/summarize-realtime.py` tổng hợp số đo; audio, phụ đề nguồn và log thô được giữ cục bộ ngoài Git.

Kiểm thử và bản build hiện tại: [PROGRESS](../PROGRESS.vi.md). Hướng dẫn toolchain và cấu hình GPU: [phát triển](../guides/development.vi.md).
