//! Exercises the actual local runtime and downloaded models without recording audio.
use nexq_lib::{
    local_runtime::{self, ModelRole},
    rag::{config::RagConfig, embedder::EmbeddingClient},
    translation::{hy_mt::HyMtTranslator, TranslationProvider},
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let runtime = local_runtime::shared();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let data = root.join(".cache/local-model-smoke");
    std::fs::create_dir_all(&data)?;
    runtime
        .initialize(data.join("local-models.json"))
        .map_err(anyhow::Error::msg)?;
    let outcome = async {
        let translation = HyMtTranslator::new(runtime.clone());
        for (text, source, target) in [("来月末までに完了できますか？", "ja", "vi"), ("Chúng tôi sẽ kiểm tra tiến độ rồi phản hồi lịch trình chính thức.", "vi", "ja")] {
            let start = std::time::Instant::now();
            let result = translation.translate(text, Some(source), target).await?;
            println!("{}", serde_json::json!({"kind":"translation", "source":source, "target":target, "result":result, "elapsed_ms":start.elapsed().as_millis()}));
        }
        let copilot = runtime.ensure(ModelRole::Copilot).await.map_err(anyhow::Error::msg)?;
        let response: serde_json::Value = reqwest::Client::builder().no_proxy().build()?.post(format!("{}/chat/completions", copilot))
            .json(&serde_json::json!({"model":"copilot", "messages":[{"role":"user","content":"Khách hỏi tiến độ dự án nhưng chưa có lịch xác nhận. Gợi ý một câu trả lời ngắn bằng tiếng Nhật rồi giải thích bằng tiếng Việt. /no_think"}],"max_tokens":192,"chat_template_kwargs":{"enable_thinking":false}}))
            .send().await?.error_for_status()?.json().await?;
        println!("{}", serde_json::json!({"kind":"copilot", "result":response["choices"][0]["message"]["content"]}));
        let config = RagConfig { embedding_provider:"local_gguf".into(), embedding_model:"embeddings".into(), embedding_dimensions:1024, ..Default::default() };
        let embedder = EmbeddingClient::from_config(&config);
        let embeddings = embedder.embed_documents(vec!["納期は来月末です。".into(), "Thời hạn là cuối tháng sau.".into(), "昼食にはラーメンを食べました。".into()], "embeddings").await.map_err(anyhow::Error::msg)?;
        anyhow::ensure!(embeddings.len() == 3 && embeddings.iter().all(|v| v.len() == 1024), "Unexpected embedding dimensions");
        let similarity: f32 = embeddings[0].iter().zip(&embeddings[1]).map(|(a,b)| a*b).sum();
        let query = embedder.embed_query("Khi nào phải giao dự án?", "embeddings").await.map_err(anyhow::Error::msg)?;
        let ranked = nexq_lib::rag::vector_store::search_similar(&query, &[("ja-deadline".into(), embeddings[0].clone()), ("vi-deadline".into(), embeddings[1].clone()), ("lunch".into(), embeddings[2].clone())], 3);
        anyhow::ensure!(ranked.first().map(|item| item.0.as_str()) != Some("lunch"), "Cross-language retrieval selected the distractor");
        println!("{}", serde_json::json!({"kind":"embeddings", "dimensions":embeddings[0].len(), "ja_vi_similarity":similarity, "ranked":ranked}));
        let (options,backend,device)=nexq_lib::stt::whisper_cpp::context_parameters();
        let context = whisper_rs::WhisperContext::new_with_params(root.join("models/whisper_cpp/ggml-large-v3-turbo.bin").to_str().unwrap(), options)?;
        anyhow::ensure!(context.is_multilingual(), "Whisper model must support multiple languages");
        println!("{}", serde_json::json!({"kind":"whisper", "model":"large-v3-turbo", "multilingual":true,"backend":backend,"device":device}));
        Ok::<_, anyhow::Error>(())
    }.await;
    runtime.shutdown().await;
    outcome
}
