import { describe, expect, it } from "vitest";
import { lastSentence, looksLikeQuestion } from "./questionDetection";

describe("Japanese and Vietnamese meeting questions", () => {
  it.each(["来月末までに完了できますか。", "ご確認いただけますか？", "問題はありますか", "Bạn có thể xác nhận được không?", "Tài liệu sẵn sàng chưa.", "Ai là người phụ trách", "Khi nào chúng ta bắt đầu", "Can you confirm the deadline?"])("detects %s", (text) => expect(looksLikeQuestion(text)).toBe(true));
  it.each(["来月完成します。", "Chúng tôi sẽ kiểm tra lại.", "The delivery is tomorrow."])("keeps statements as statements: %s", (text) => expect(looksLikeQuestion(text)).toBe(false));
  it("finds the last Japanese sentence in a growing transcript line", () => {
    expect(lastSentence("資料を確認しました。来月までに完了できますか？")).toBe("来月までに完了できますか？");
  });
});
