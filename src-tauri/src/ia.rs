//! Conversa com o Ollama (IA local).
//!
//! Por que pelo Rust e não com `fetch` no TypeScript?
//! O front roda numa origem tipo `http://tauri.localhost`; o Ollama em
//! `localhost:11434` só aceita origens que estejam na variável OLLAMA_ORIGINS,
//! então um fetch do front esbarraria no CORS. O Rust não é navegador, então
//! não existe CORS aqui. De quebra, o front não precisa de permissão de rede.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::EstadoConfig;

/// Corpo da requisição para POST /api/chat do Ollama.
/// Só os campos que usamos; o resto fica no padrão do Ollama.
#[derive(Serialize)]
struct PedidoChat<'a> {
    model: &'a str,
    messages: Vec<Mensagem<'a>>,
    /// false = resposta inteira de uma vez (mais simples que streaming).
    stream: bool,
    /// JSON Schema da resposta. O Ollama usa isso para OBRIGAR o modelo a gerar
    /// JSON nesse formato ("structured outputs"). Ajuda muito modelos pequenos,
    /// mas NÃO substitui a validação com zod no front.
    format: serde_json::Value,
    /// Quanto tempo o modelo fica carregado na memória depois do uso.
    keep_alive: &'a str,
    options: Opcoes,
}

#[derive(Serialize)]
struct Mensagem<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct Opcoes {
    /// Criatividade (0 = sempre igual, 1+ = mais variado).
    temperature: f32,
    /// Limite de tokens na resposta: a fala é curta, então isso evita
    /// o modelo "desembestar" e ficar gerando por muito tempo.
    num_predict: u32,
}

#[derive(Deserialize)]
struct RespostaChat {
    message: MensagemResposta,
}

#[derive(Deserialize)]
struct MensagemResposta {
    content: String,
}

/// Comando chamado pelo front: `invoke("ask_llm", { sistema, prompt, formato })`.
///
/// Devolve o TEXTO CRU que o modelo gerou. Quem interpreta e valida é o TypeScript
/// (com zod), porque as regras de "o que é uma resposta aceitável" ficam lá.
#[tauri::command]
pub async fn ask_llm(
    // `State` é injetado pelo Tauri (não vem do front). O `'_` é um "lifetime":
    // diz ao Rust que essa referência só vale durante esta chamada.
    estado: tauri::State<'_, EstadoConfig>,
    sistema: String,
    prompt: String,
    formato: serde_json::Value,
) -> Result<String, String> {
    // Copiamos a config ANTES do `.await` abaixo, para não segurar o lock
    // durante os segundos em que o Ollama está pensando.
    let cfg = estado.ler();

    // Um cliente por chamada é ok aqui (o Buddy fala de vez em quando).
    // O timeout é longo porque a PRIMEIRA chamada carrega o modelo na memória,
    // o que pode levar dezenas de segundos sem placa de vídeo.
    let cliente = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let pedido = PedidoChat {
        model: &cfg.modelo,
        messages: vec![
            Mensagem { role: "system", content: &sistema },
            Mensagem { role: "user", content: &prompt },
        ],
        stream: false,
        format: formato,
        keep_alive: "10m",
        options: Opcoes { temperature: 0.7, num_predict: 200 },
    };

    let resposta = cliente
        .post(format!("{}/api/chat", cfg.ollama_url))
        .json(&pedido)
        .send()
        .await
        .map_err(|e| format!("Ollama não respondeu ({}). Ele está aberto? Detalhe: {e}", cfg.ollama_url))?;

    if !resposta.status().is_success() {
        let status = resposta.status();
        let corpo = resposta.text().await.unwrap_or_default();
        // Erro mais comum: modelo não baixado → status 404 com "model not found".
        return Err(format!("Ollama devolveu {status}: {corpo}"));
    }

    let dados: RespostaChat = resposta.json().await.map_err(|e| e.to_string())?;
    Ok(dados.message.content)
}

/// O Ollama está no ar? Usado pelo LED de status (vermelho = fora do ar).
///
/// Bate em GET /api/version, a rota mais leve do Ollama (não carrega modelo
/// nenhum). Timeout curto: se ele não responder em 2s, para o Buddy é "fora".
#[tauri::command]
pub async fn ollama_ok(estado: tauri::State<'_, EstadoConfig>) -> Result<bool, String> {
    let url = format!("{}/api/version", estado.ler().ollama_url);
    let cliente = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(cliente
        .get(url)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false))
}
