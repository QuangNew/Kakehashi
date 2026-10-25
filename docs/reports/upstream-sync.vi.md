> Ghi nhận tích hợp chọn lọc từ NexQ; các số kiểm thử cuối báo cáo thuộc mốc trước khi tối ưu realtime. Kết quả hiện tại: [PROGRESS](../PROGRESS.vi.md).

# Kakehashi: các bản sửa chọn từ NexQ 2.25

Đối chiếu ngày 24/10/2026 giữa nền NexQ 2.24.0 (`e42b2ca`) và `origin/main` ở bản 2.25.0 (`f24ca46`). Có 28 commit mới; đây là tích hợp chọn lọc, không phải cập nhật toàn bộ dự án lên NexQ 2.25.

| Nguồn upstream | Đã áp dụng cho Kakehashi |
| --- | --- |
| [35cd0c6 — Audio Settings](https://github.com/naxhq/NexQ/commit/35cd0c6495a29090e607de6376b14896a8f54d9f) | Chờ cả danh sách model và trạng thái API key trước khi xét tính khả dụng; mở Settings không tự thay provider đã lưu. Không tự chuyển hai nguồn sang engine chỉ dùng được cho một nguồn. |
| [09dc9a0 — Custom LLM](https://github.com/naxhq/NexQ/commit/09dc9a0a16204741c71c82bd8cbf3fe3b9c79f1b) cùng phần hoàn thiện ở 2.25 | Giữ URL, loại xác thực và model của API tùy chỉnh qua lần mở lại. Token tiếp tục được lưu bằng CredentialManager, không đưa vào file config. Các cửa sổ dùng chung URL/loại xác thực đã lưu. |
| [429f326 — Meeting reset](https://github.com/naxhq/NexQ/commit/429f32611f73dee2687540a813d88e600c86f60e) cùng phần hoàn thiện ở 2.25 | Xóa transcript, câu trả lời, người nói, bookmark, bản dịch và trạng thái phát lại trước cuộc họp mới ở cả launcher lẫn overlay. Khởi tạo lại người nói và remount giao diện theo ID cuộc họp. Thêm việc xóa lỗi/progress dịch cùng session toggle của bản song ngữ. |
| [6514b61 — Document fallback](https://github.com/naxhq/NexQ/commit/6514b61bd32a42b542d9e4bd3fc4edd07d0609a7) cùng phần giới hạn ở 2.25 | Khi người dùng bật tài liệu cho AI action nhưng RAG không tìm được chunk, đưa tài liệu đính kèm vào prompt. Bản local giới hạn theo context còn lại, dự trữ phần trả lời và giữ nguyên biên UTF-8; tránh đưa 32 nghìn ký tự vào model có context 4096. Token Budget cũng đọc context copilot local đã cấu hình. |
| [3be41ca — Deepgram reconnect](https://github.com/naxhq/NexQ/commit/3be41ca60b672afb08f7e69faba496264aa46417) | Hủy cờ của kết nối cũ và cấp cờ riêng cho kết nối mới, tránh chồng reader sau reconnect nếu chọn dùng Deepgram. |

Chưa nhập phần macOS, website, workflow phát hành và stealth mode mới vì không cần cho cấu hình Windows/phiên dịch local hiện tại. Các phiên bản và giấy phép của nền NexQ được giữ để truy vết nguồn; không bật trình cập nhật tự động của upstream.

Tên hiển thị, icon cầu nối và file chạy đổi thành **Kakehashi** / `Kakehashi.exe`. Giữ nguyên application ID, schema SQLite, tên file cấu hình, event/IPC nội bộ và vị trí model để tránh tạo bộ dữ liệu mới chỉ vì đổi tên. Giấy phép MIT và ghi nhận NexQ được giữ tại LICENSE, README và About.

Kiểm tra EXE cũng phát hiện Token Budget ở giao diện giữ giá trị 128k từ trước khi backend khôi phục provider local. Đã thêm việc làm mới sau khi provider sẵn sàng, khi đổi provider ở cửa sổ khác và khi lưu context local; regression test mô phỏng khởi động chậm và đổi local/API giữa hai cửa sổ.

Script build so SHA-256 trước khi chép DLL: dùng lại file giống hệt đang được Windows giữ, vẫn báo lỗi nếu file bị khóa có nội dung khác. EXE mang tên cũ đã được lưu tại `.cache/build-backups/`.

Xác minh ngày 24/10/2026: TypeScript, 267 test frontend / 43 file, 247 test Rust và build Windows đều đạt. EXE Kakehashi mở được khi PATH chỉ chứa thư mục Windows; giữ thiết lập Whisper Small Việt/Nhật, copilot và RAG local 1024 chiều. Token Budget trên giao diện và IPC đều đúng 4096. Dịch Nhật→Việt và copilot đọc tài liệu chưa lập chỉ mục chạy bằng model thật; fixture được gỡ sau kiểm tra, không có JavaScript exception. Đối chiếu 17 bảng SQLite cùng thiết lập đã lưu và kiểm tra dừng server khi thoát bình thường đều đạt. Chưa thử âm thanh cuộc họp thực tế.
