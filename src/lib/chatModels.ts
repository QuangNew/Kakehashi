// The provider catalog may include audio/embedding endpoints beside chat.
// Keep unknown IDs selectable; exclude only recognized non-chat families.
export function isChatModel(id:string):boolean {
  const name=id.toLowerCase();
  return !["whisper","transcribe","text-embedding","nomic-embed","all-minilm","mxbai-embed","bge-","jina-embedding","snowflake-arctic-embed","tts-","playai-tts","orpheus"].some(pattern=>name.includes(pattern));
}
