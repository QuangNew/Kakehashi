# Dùng Kakehashi

1. Mở `out/windows/Kakehashi.exe`. Lần đầu có hướng dẫn; lần sau vào Settings để đổi cấu hình.
2. **Audio & Devices:** chọn microphone của bạn và thiết bị đầu ra dùng để nghe khách. Preset mặc định: mic Việt, system Nhật, Whisper Large v3 Turbo. Có preset đảo hai ngôn ngữ. Dùng tai nghe để hạn chế âm thanh của khách lọt vào mic.
3. **API Pool:** thêm key một lần, đặt tên gợi nhớ. Có thể lưu nhiều key cùng nhà cung cấp. Màu xanh nghĩa là đã lưu key và có thể chọn; kiểm tra kết nối để xác nhận quyền truy cập catalog. Khi chọn provider thiếu key, ứng dụng hỏi có muốn mở API Pool. Key được giữ trong Windows Credential Manager; file `api-pool.json` chỉ chứa nhãn và các lựa chọn. Key cũ được nhập tự động một lần.
4. **Translation:** chọn Hy-MT2 local, Groq hoặc Google AI Studio. Với API, chọn key trong Pool, nạp danh sách model, chọn model và bấm **Áp dụng AI dịch**. Copilot ở **LLM Providers** chọn provider/model độc lập; có thể dùng chung key hoặc chọn key khác. Giữ Ollama, LM Studio, Claude subscription, các API copilot và các provider dịch kế thừa.
5. Đính kèm tài liệu tại dashboard nếu cần. Context Strategy chọn tài liệu trực tiếp hoặc RAG local; đổi file embedding tại Local Models, rồi dựng lại chỉ mục.
6. Chọn online hoặc in-person và scenario Meeting/Interview/Lecture/Webinar/Oral Exam, rồi đợi chuẩn bị model hoàn tất. In-person dùng microphone của bạn làm mic chung, nhận dạng bằng provider của nguồn Room/Them. Whisper chốt theo khoảng lặng hoặc cap mặc định 3 giây rồi gửi dịch. Thanh trạng thái hiện GPU/CPU, tên thiết bị khi rê chuột và độ trễ; số trễ chưa gồm thời gian gom đoạn. Copilot dùng AI Actions hoặc câu hỏi bạn nhập. Các AI action có cửa sổ transcript, tài liệu và prompt riêng.
7. Kết thúc cuộc họp trước khi đổi thiết bị. Trong lịch sử có transcript, bản dịch đã lưu, AI log, hỏi lại về cuộc họp và nghe ghi âm nếu đã bật lưu.

Các ô **Kiểm tra kết nối** kiểm tra truy cập/cấu hình, không thay thế kiểm thử chất lượng dịch. Danh sách model được lấy từ key đã chọn; chọn model hỗ trợ chat/text. API Pool có lựa chọn key riêng cho STT, AI dịch và copilot. Xóa key đang dùng sẽ yêu cầu chọn lại; ứng dụng không tự nhảy sang tài khoản khác.

## Lưu thiết lập và vào họp nhanh hơn

Thiết lập được lưu sau mỗi thay đổi; **Lưu ngay** trong Settings ghi ngay các thay đổi còn chờ và cho phép thử lại nếu ghi lỗi. Cấu hình file model và AI dịch có bản nháp riêng: đổi tab hoặc khởi động lại không làm mất bản nháp, nhưng cần **Áp dụng** để đổi backend. **Đang dùng** hiển thị cấu hình đã được backend chấp nhận.

Ở **Local Models → Chuẩn bị model trước cuộc họp**, bật **Tự chuẩn bị model local khi mở ứng dụng** hoặc bấm **Chuẩn bị ngay**. Ứng dụng nạp model và làm nóng GPU mà không thu âm, tạo cuộc họp hay gửi nội dung lên cloud. Sau đó vào họp sẽ dùng lại model đã chuẩn bị. Whisper và runtime dịch được giữ giữa các cuộc họp; **Giải phóng STT / dịch** hoặc thoát ứng dụng để trả bộ nhớ. Tính năng này dùng thêm RAM/VRAM khi chờ.

Lần nạp đầu vẫn cần đọc trọng số và chuẩn bị GPU. Chuẩn bị nền chuyển công việc đó sang trước lúc vào họp; nó không loại bỏ thời gian nạp vật lý. Tắt chuẩn bị tự động nếu cần GPU cho việc khác.

Thiết lập nhanh có nút **Đóng** trên mọi bước, **Thiết lập sau** và phím **Esc**. Chạy lại hướng dẫn trong Settings không xóa cấu hình đã lưu.

Whisper luôn xử lý local. Khi chọn API, phần văn bản dùng để dịch hoặc tạo gợi ý được gửi tới nhà cung cấp tương ứng. Giới hạn, phí và quota phụ thuộc tài khoản của bạn; ứng dụng không giả định API luôn miễn phí.

Tài liệu API: [Groq](https://console.groq.com/docs/api-reference#chat-create), [Gemini generateContent](https://ai.google.dev/api/generate-content), [model catalog Gemini](https://ai.google.dev/api/models).
