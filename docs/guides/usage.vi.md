# Dùng Kakehashi

1. Mở `out/windows/Kakehashi.exe`. Lần đầu có hướng dẫn; lần sau vào Settings để đổi cấu hình.
2. **Audio & Devices:** chọn microphone của bạn và thiết bị đầu ra dùng để nghe khách. Preset mặc định: mic Việt, system Nhật, Whisper Small. Có preset đảo hai ngôn ngữ. Dùng tai nghe để hạn chế âm thanh của khách lọt vào mic.
3. **Translation:** chọn Hy-MT2 local, Groq hoặc Google AI Studio. Với API, nhập key dành cho dịch, nạp danh sách model, chọn model và bấm **Áp dụng cấu hình AI dịch**. Các lựa chọn Microsoft/Google Cloud Translation, DeepL, OPUS-MT và dùng chung copilot vẫn được giữ. Bật dịch hai chiều và chọn cách hiển thị phụ đề.
4. **LLM Providers:** chọn copilot, key và model riêng, rồi đặt làm provider đang dùng. Có thể dùng model trong folder, Ollama, LM Studio, Claude subscription hoặc API. Chỉ tùy chọn dịch **LLM dùng chung provider/model copilot** mới sử dụng cấu hình của copilot; Groq/Gemini dành cho dịch vẫn độc lập.
5. Đính kèm tài liệu tại dashboard nếu cần. Context Strategy chọn tài liệu trực tiếp hoặc RAG local; đổi file embedding tại Local Models, rồi dựng lại chỉ mục.
6. Chọn online hoặc in-person và scenario Meeting/Interview/Lecture/Webinar/Oral Exam, rồi đợi chuẩn bị model hoàn tất. In-person dùng microphone của bạn làm mic chung, nhận dạng bằng provider của nguồn Room/Them. Whisper chốt theo khoảng lặng hoặc cap mặc định 3 giây rồi gửi dịch. Thanh trạng thái hiện GPU/CPU, tên thiết bị khi rê chuột và độ trễ; số trễ chưa gồm thời gian gom đoạn. Copilot dùng AI Actions hoặc câu hỏi bạn nhập. Các AI action có cửa sổ transcript, tài liệu và prompt riêng.
7. Kết thúc cuộc họp trước khi đổi thiết bị. Trong lịch sử có transcript, bản dịch đã lưu, AI log, hỏi lại về cuộc họp và nghe ghi âm nếu đã bật lưu.

Các ô **Kiểm tra kết nối** kiểm tra truy cập/cấu hình, không thay thế kiểm thử chất lượng dịch. Danh sách model được lấy từ key của nhà cung cấp; chọn model hỗ trợ chat/text. Key Groq và Gemini cho dịch có namespace riêng, không tự lấy từ copilot.

Whisper luôn xử lý local. Khi chọn API, phần văn bản dùng để dịch hoặc tạo gợi ý được gửi tới nhà cung cấp tương ứng. Giới hạn, phí và quota phụ thuộc tài khoản của bạn; ứng dụng không giả định API luôn miễn phí.

Tài liệu API: [Groq](https://console.groq.com/docs/api-reference#chat-create), [Gemini generateContent](https://ai.google.dev/api/generate-content), [model catalog Gemini](https://ai.google.dev/api/models).
