export function looksLikeQuestion(text: string): boolean {
  const trimmed = text.trim();
  if (/[?？]$/u.test(trimmed)) return true;
  const lower = trimmed.toLowerCase().replace(/[。.!！]+$/u, "").trim();
  if (/(?:ですか|ますか|ませんか|でしょうか|だろうか|なのか)$/u.test(lower)) return true;
  if (/^(?:tại sao|vì sao|khi nào|bao giờ|bao nhiêu|làm thế nào|ở đâu|ai là)(?:\s|$)/u.test(lower)) return true;
  if (/(?:được không|đúng không|phải không|hay không|được chứ|chưa)$/u.test(lower)) return true;
  return ["what ", "how ", "why ", "when ", "where ", "who ", "which ", "can you", "could you", "would you", "do you", "are you", "is there", "have you", "tell me", "explain"].some((word) => lower.startsWith(word));
}

export function lastSentence(text: string): string {
  const sentences = text.trim().match(/[^.?!。！？]+[.?!。！？]*/gu) ?? [];
  return (sentences[sentences.length - 1] ?? "").trim();
}
