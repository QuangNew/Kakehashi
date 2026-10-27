# Chuyển hoàn toàn sang workspace Kakehashi — 27/10/2026

Workspace chạy và phát triển hiện là `Kakehashi`; thư mục `T_VNJP` cũ đã được xóa sau khi kiểm tra bản sao lưu. Model, runtime và dữ liệu cá nhân không được đưa vào Git.

## Các phần đã chuyển

- `models/`: Whisper Large v3 Turbo, Hy-MT2 7B và 1.8B, Qwen3 1.7B copilot, Qwen3 Embedding 0.6B.
- `runtimes/`: llama.cpp Vulkan và các DLL đi kèm.
- `.native-tools/`: CMake, Ninja, libclang, công cụ Vulkan và adapter GNU.
- `src-tauri/ort/`: thư mục DLL thật; đã bỏ junction nối sang dự án cũ.
- Cấu hình `local-models.json` trong AppData đã đổi hai đường dẫn model/runtime sang Kakehashi. Các lựa chọn provider, model, ngôn ngữ, API Pool và dữ liệu ứng dụng được giữ.
- Backup và số đo cũ được giữ trong `.cache/legacy-T_VNJP`; audio mẫu trong `.cache/realtime`. Các tài liệu lịch sử có thể còn ghi đường dẫn cũ, không phải phụ thuộc đang chạy.

Đã đối chiếu SHA-256 của 4.120 file trước/sau khi chuyển. Sau đó xóa các bộ tải SDK tạm và phần tải Whisper dang dở vì model hoàn chỉnh đã khớp checksum được ghim.

## Dọn dung lượng

Đã xóa cache native cũ khoảng 37,21 GiB, node_modules/dist/out cũ, bản tải Turbo dư, archive tải công cụ và bốn EXE backup đã lỗi thời. Cache build mới đang dùng trong Kakehashi được giữ để tiếp tục phát triển.

Bản ZIP mã nguồn và toàn bộ Git cũ được kiểm tra 1.517 file trước khi xóa thư mục gốc. Bản sao cục bộ nằm tại:

`Kakehashi/.cache/migration-20261027/T_VNJP-source-and-git.zip`

Kết quả đối chiếu, cấu hình trước chuyển và log kiểm thử nằm cùng thư mục; không được commit. File ZIP này khoảng 29 MB. Thư mục T_VNJP đã được xác nhận không còn tồn tại.

## Kiểm chứng

- `pnpm build` và `pnpm desktop:build` đạt, dùng công cụ và cache hoàn toàn dưới Kakehashi. Cache build cũ đã bị xóa trước khi build lại.
- `pnpm models:smoke` đạt: dịch Nhật→Việt, Việt→Nhật, copilot local, embedding 1.024 chiều và Whisper Turbo đa ngôn ngữ trên RTX 4050 qua Vulkan. Smoke helper đã đổi từ Whisper Small sang Turbo và dùng cùng cách chọn GPU với ứng dụng.
- Khởi động EXE và kiểm tra IPC sau khi T_VNJP đã bị xóa: đúng model/runtime mới, nhận đủ Whisper Turbo, API Pool còn nguyên và không có JavaScript exception.
- Không còn junction/symlink trong dự án mới nối sang T_VNJP. Không có tham chiếu đường dẫn tuyệt đối cũ trong source/script đang chạy.
- Không tạo cuộc họp, ghi âm hay gọi AI cloud trong các phép thử này. Runtime model do phép thử tạo đều được đóng sau khi hoàn tất.

Bản chạy: `out/windows/Kakehashi.exe`. Mở `Kakehashi.code-workspace` để phát triển và Commit/Pull vào `QuangNew/Kakehashi`.
