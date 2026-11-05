# Timestamp, dịch trực tiếp và khởi động — 05/11/2026

Đã sửa đồng hồ transcript và cho bản dịch Hy-MT2 hiện dần khi model sinh văn bản. Giữ Whisper Large v3 Turbo và Hy-MT2 7B Q4, cùng prompt, ngữ cảnh và tham số lấy mẫu. Lỗi tự đóng sau khi hiện màn hình chính chưa tái hiện được; đã gia cố thứ tự khởi tạo và bổ sung log lưu trên máy.

## Đồng hồ transcript

Whisper trả thời gian tương đối từ lúc capture, trong khi UI, lịch sử và playback dùng mili giây epoch. Phép trừ hai hệ thời gian làm kết quả âm và bị chặn thành `0:00.000`. Relay hai nguồn âm thanh giờ đổi thời gian tương đối sang epoch bằng mốc capture chung, đồng thời giữ nguyên timestamp vốn đã là epoch của provider khác. Hai webview lấy mốc cuộc họp từ `meeting.start_time` đã lưu.

Test hồi quy bao gồm offset bằng 0, `12.345` giây, `65.432` giây, timestamp epoch có sẵn và khởi động lại capture giữa cuộc họp. Kiểm tra GUI dùng sự kiện transcript tổng hợp với các mốc đã biết; việc chuyển thời gian trong relay được kiểm tra riêng bằng test Rust.

## Dịch hiện dần và bộ nhớ

Hy-MT2 dùng SSE để gửi văn bản đang sinh lên overlay, tối đa khoảng 20 cập nhật/giây. Bộ đọc giữ đúng UTF-8 dù ký tự Nhật/Việt bị chia qua nhiều gói mạng. Chỉ bản dịch kết thúc hợp lệ mới được lưu; phần dở bị hủy, lỗi hoặc hết giới hạn token không trở thành bản dịch hoàn chỉnh. Preview của cuộc họp cũ hoặc revision cũ bị bỏ qua.

Trên Windows, runtime dịch local có GPU dùng `LLAMA_ARG_LOAD_MODE=none` nếu người dùng chưa đặt biến này. Binary llama-server đang cài có hỗ trợ tùy chọn đó. Trong các lần đo, working set của tiến trình dịch giảm từ khoảng 5,2 xuống 3,1 GiB; bộ nhớ trống tăng từ khoảng 0,4 lên 3,3 GiB. Đây là quan sát trên máy thử, không phải mức RAM cố định cho mọi lần chạy. Giữ cấu hình thật ở **22 GPU layers**; thử 24 layers không giải quyết được hàng đợi dịch và làm STT chậm hơn.

Streaming giảm thời gian chờ thấy chữ đầu tiên. Tổng thời gian sinh và khả năng xử lý liên tục vẫn phụ thuộc tốc độ model; streaming không làm biến mất hàng đợi.

## Đo lại podcast trên máy người dùng

Máy: i7-13700HX, RAM 16 GB, RTX 4050 Laptop 6 GB, Windows. Phát lại 90 giây, đoạn **00:30–02:00** của [video mẫu](https://www.youtube.com/watch?v=SmxSCZzV7j0), tốc độ 1×. Chạy Whisper Turbo/Vulkan và dịch Hy-MT2 7B Q4, thêm nguồn VI im lặng dùng chung trọng số Whisper. Không build trong lúc đo; những ứng dụng khác của người dùng vẫn hoạt động. Bài đo đi qua provider thật, chưa gồm WASAPI và thời gian vẽ UI.

| Chỉ số | Mmap mặc định, 22 layers | Load mode none, 22 layers | Load mode none, 24 layers thử nghiệm |
| --- | ---: | ---: | ---: |
| Transcript cuối | 39 | 39 | 39 |
| Audio bị bỏ | 0 | 0 | 0 |
| STT sau cuối đoạn, trung vị / p95 | 1.001 / 10.772 ms | 913 / 1.131 ms | 1.095 / 1.421 ms |
| CER tiếng Nhật | 13,87% | 13,87% | 13,87% |
| Dịch xong trước hạn dừng bài đo | 29/39 | 23/39 | 23/39 |
| Chữ đầu sau khi bắt đầu request, trung vị | 602 ms | 582 ms | 700 ms |
| Sinh bản dịch hoàn chỉnh, trung vị | 3.346 ms | 5.420 ms | 5.108 ms |
| Chờ hàng đợi dịch, trung vị | 52.145 ms | 43.036 ms | 41.954 ms |
| Dịch hoàn chỉnh sau cuối audio, trung vị / p95 | 62.479 / 67.862 ms | 47.008 / 72.601 ms | 46.135 / 72.778 ms |

CER là 52 phép sửa trên 375 ký tự tham chiếu sau chuẩn hóa, giống nhau ở cả ba lần. Đây là kiểm tra STT; chưa có chấm điểm ngữ nghĩa bản dịch Việt bởi người dịch. Tham số lấy mẫu của Hy-MT2 không đổi, nhưng nội dung và độ dài đầu ra giữa các lần có thể khác nhau.

Bài đo cho STT thêm 10 giây và dịch thêm 30 giây để xử lý phần còn lại, rồi chủ động tắt runtime. Những request chưa hoàn tất lúc đó ghi lỗi hủy khi shutdown (10 hoặc 16 request); không diễn giải chúng thành lỗi provider phát sinh trong lúc dịch bình thường. Các chỉ số dịch trong bảng chỉ tính request đã hoàn tất, nên không coi đây là kết quả đầy đủ cho cả 39 đoạn.

Kết quả hiện tại: STT theo kịp mẫu khi giảm áp lực RAM, còn Hy-MT2 7B local vẫn tích hàng đợi. Chưa đạt phiên dịch realtime liên tục trên máy và tải nền của lần thử này. Các số đo ngày 27/10 trong báo cáo cũ là mốc lịch sử, không phải kết quả của lần chạy này và không đủ để khẳng định mức cải thiện tương đối.

Nếu ưu tiên theo kịp cuộc họp liên tục, có thể giữ Whisper local và chọn dịch Groq hoặc Google AI Studio trong cấu hình hiện có. Chưa tự chuyển provider hay gọi API bằng key của người dùng. Việc đổi model/quantization nhỏ hơn có thể giảm chi phí tính toán nhưng cần đánh giá chất lượng riêng, nên chưa áp dụng.

## Khởi động

Các webview chỉ được tạo sau khi database, settings và dịch vụ backend đã được đăng ký. Launcher được hiện sau khi hoàn tất tray và các handler cửa sổ. Trước đây React có thể bắt đầu gọi backend khi các dịch vụ còn đang khởi tạo; thay đổi này loại bỏ thứ tự không an toàn đó, nhưng chưa chứng minh đây là nguyên nhân lần tự đóng mà người dùng gặp.

Log khởi động, thao tác hiện/ẩn cửa sổ, exit và Rust panic được lưu ở `%APPDATA%\com.tvnjp.app\logs\app.log`, xoay vòng với `app.previous.log`, mỗi file khoảng 2 MiB. Lỗi ghi log không ngăn ứng dụng hoạt động. Panic hook không thay thế crash dump cho lỗi native.

Khi bắt đầu khảo sát, có tiến trình app vẫn chạy với cửa sổ bị ẩn. Windows Application log không có bản ghi crash mới khớp ứng dụng; bản EXE cũ mở lại 30 giây không tái hiện lỗi. Vì vậy chưa kết luận app đã crash hay chỉ ẩn xuống tray trong lần người dùng gặp.

## Kiểm chứng

- 293 test frontend và 272 test Rust đạt.
- Build frontend và EXE Windows với Vulkan đạt. Hai lần mở tiến trình mới của EXE cuối đều giữ launcher hiển thị, sẵn sàng trong 30 giây; không có JavaScript exception. Các lần đóng có log exit bình thường. Chưa tái hiện được lỗi tự đóng ban đầu, cũng chưa kiểm tra lại sau reboot Windows.
- Overlay thật hiển thị hai mốc `0:12.345` và `1:05.432` đúng theo sự kiện tổng hợp. Dùng model dịch thật: câu một thấy preview sau 1.052 ms, hoàn tất sau 4.151 ms; câu hai tương ứng 640 và 4.909 ms. Đây là hai câu gửi lần lượt, không đại diện cho throughput của podcast liên tục. Ảnh overlay và kết quả máy đọc được lưu riêng trong `.cache`.
- Chuẩn bị Whisper Turbo hai nguồn và runtime dịch trong lần GUI cuối mất 18.212 ms, có một bộ trọng số Whisper dùng chung. Lần mở tiến trình mới vẫn phải đọc/nạp model; giữ runtime và chuẩn bị trước cuộc họp tiếp tục là cách giảm thời gian chờ khi bấm bắt đầu.
- Dữ liệu thử, log, audio và model nằm ngoài Git. Chỉ xóa cuộc họp tổng hợp do bài kiểm tra tạo; giữ cấu hình và model của người dùng.
