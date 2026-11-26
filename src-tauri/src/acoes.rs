//! Ações que o Buddy pode executar: uma ALLOWLIST FECHADA.
//!
//! Por que validar aqui de novo, se o TypeScript já valida com zod?
//! Defesa em profundidade: se um dia houver um bug no front (ou alguém injetar
//! JavaScript nele), o máximo que dá para pedir ao Rust é uma das ações deste
//! `enum`. Não existe "abrir URL qualquer" nem "rodar comando": as URLs são
//! montadas AQUI, e o texto do usuário/modelo só entra como parâmetro de busca.

use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use crate::config::EstadoConfig;

/// `#[serde(tag = "tipo")]` faz o JSON `{ "tipo": "abrir_busca", "consulta": "..." }`
/// virar `Acao::AbrirBusca { consulta }`. Qualquer outro "tipo" falha ao
/// desserializar, então ações inventadas são recusadas automaticamente.
#[derive(Deserialize, Debug)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Acao {
    AbrirBusca { consulta: String },
}

const MAX_CONSULTA: usize = 100;

/// Vídeos que o Buddy abriu por último. Bug real: ele sugeriu Madeon, abriu um
/// set do Madeon; depois sugeriu Madeon de novo, e o 1º resultado era O MESMO
/// vídeo que já estava tocando. Agora esses são pulados.
static ABERTOS: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
const MAX_ABERTOS: usize = 30;

/// Comando chamado pelo front SOMENTE depois do clique em "Sim".
///
/// `pausar`: se true, pausa a música ANTES de abrir a sugestão (como no "lemme
/// pause this" do vídeo). O front só manda true quando o balão AVISOU
/// "⏸ pauso a música", então o seu "Sim" cobre exatamente isso.
///
/// É `async` porque pausar e achar o vídeo esperam respostas (do Windows e do
/// YouTube); rodando na thread da janela, isso travaria o Buddy.
#[tauri::command]
pub async fn executar_acao(
    estado: tauri::State<'_, EstadoConfig>,
    acao: Acao,
    pausar: bool,
) -> Result<(), String> {
    // Destino e buscador vêm da config, já validada em config.rs.
    let cfg = estado.ler();

    if pausar {
        // Se não conseguir pausar, seguimos mesmo assim (só registra).
        match tauri::async_runtime::spawn_blocking(crate::contexto::pausar_midia).await {
            Ok(Ok(pausou)) => println!("[buddy] pausar mídia: {pausou}"),
            outro => eprintln!("[buddy] não consegui pausar a mídia: {outro:?}"),
        }
    }

    match acao {
        Acao::AbrirBusca { consulta } => {
            let consulta = limpar_consulta(&consulta)?;
            let url = match cfg.destino_sugestoes.as_str() {
                "youtube_tocar" => match primeiro_video(&consulta, titulo_tocando().await).await {
                    Some(id) => format!("https://www.youtube.com/watch?v={id}"),
                    // Plano B: o YouTube mudou a página, ou não achou nada.
                    None => url_busca_youtube(&consulta)?,
                },
                "youtube_busca" => url_busca_youtube(&consulta)?,
                _ => url_busca_web(&cfg.buscador, &consulta)?,
            };
            if let Some(id) = url.strip_prefix("https://www.youtube.com/watch?v=") {
                if let Ok(mut abertos) = ABERTOS.lock() {
                    abertos.push_back(id.to_string());
                    if abertos.len() > MAX_ABERTOS {
                        abertos.pop_front();
                    }
                }
            }
            println!("[buddy] abrindo: {url}");
            open::that_detached(&url).map_err(|e| format!("não consegui abrir o navegador: {e}"))
        }
    }
}

fn limpar_consulta(consulta: &str) -> Result<String, String> {
    let c: String = consulta.chars().filter(|c| !c.is_control()).take(MAX_CONSULTA).collect();
    let c = c.trim().to_string();
    if c.is_empty() {
        return Err("consulta vazia".into());
    }
    Ok(c)
}

/// `parse_with_params` codifica o texto: "Fred again.." → "Fred+again..",
/// "&" vira "%26" etc. Assim a consulta nunca "escapa" do parâmetro.
fn url_busca_web(buscador: &str, consulta: &str) -> Result<String, String> {
    let url = url::Url::parse_with_params(buscador, &[("q", consulta)]).map_err(|e| e.to_string())?;
    // Garantia extra: só abrimos https (nada de file:, javascript:, etc.).
    if url.scheme() != "https" {
        return Err("URL de busca inválida".into());
    }
    Ok(url.into())
}

fn url_busca_youtube(consulta: &str) -> Result<String, String> {
    url::Url::parse_with_params("https://www.youtube.com/results", &[("search_query", consulta)])
        .map(String::from)
        .map_err(|e| e.to_string())
}

/// Acha o id do PRIMEIRO vídeo da busca do YouTube, sem API e sem chave.
///
/// Como: baixa a página de resultados (a mesma que o navegador baixaria) e
/// procura o primeiro `"videoRenderer":{"videoId":"XXXXXXXXXXX"`, que é como o
/// YouTube descreve cada vídeo da lista dentro do HTML.
///
/// PONTO FRACO (assumido): não é uma API oficial. Se o YouTube mudar o formato
/// da página, isto devolve `None` e quem chamou abre a busca normal. Nunca
/// abre nada que não seja youtube.com/watch?v=<id validado>.
/// Título do que está tocando agora (para não reabrir o mesmo vídeo).
async fn titulo_tocando() -> Option<String> {
    tauri::async_runtime::spawn_blocking(crate::contexto::titulo_midia).await.ok().flatten()
}

async fn primeiro_video(consulta: &str, evitar_titulo: Option<String>) -> Option<String> {
    let html = baixar_resultados(consulta).await?;
    let abertos: Vec<String> = ABERTOS.lock().map(|a| a.iter().cloned().collect()).unwrap_or_default();
    let id = escolher_video(&html, evitar_titulo.as_deref(), &abertos);
    if id.is_none() {
        eprintln!("[buddy] não achei vídeo na página do YouTube (formato mudou?)");
    }
    id
}

/// Um resultado da busca do YouTube.
#[derive(Debug, PartialEq)]
struct Video {
    id: String,
    titulo: String,
    /// Número de visualizações (0 se não deu para ler, ex.: vídeo ao vivo).
    views: u64,
}

/// Baixa a página de resultados do YouTube para a busca.
async fn baixar_resultados(consulta: &str) -> Option<String> {
    let url = url_busca_youtube(consulta).ok()?;
    let cliente = reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        // Com um User-Agent incompleto, o YouTube devolve uma página SEM a lista
        // de vídeos (visto no teste). Precisa parecer um navegador de verdade.
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36")
        .build()
        .ok()?;
    cliente
        .get(url)
        .header("Accept-Language", "pt-BR,pt;q=0.9")
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()
}

/// Os primeiros vídeos da página de resultados.
/// Separado da parte de rede para dar para testar sem internet.
fn extrair_videos(html: &str) -> Vec<Video> {
    const MARCA: &str = "\"videoRenderer\":{\"videoId\":\"";
    const TITULO: &str = "\"title\":{\"runs\":[{\"text\":\"";
    const VIEWS: &str = "\"viewCountText\":{\"simpleText\":\"";
    let mut videos = Vec::new();
    let mut resto = html;
    while let Some(pos) = resto.find(MARCA) {
        resto = &resto[pos + MARCA.len()..];
        let id: String = resto.chars().take(11).collect();
        // Id do YouTube: exatamente 11 caracteres de [A-Za-z0-9_-]. Qualquer outra
        // coisa é descartada: nada de montar URL com lixo vindo de fora.
        let valido = id.len() == 11 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valido {
            continue;
        }
        // O bloco deste vídeo vai até o próximo vídeo (medido: ~13 mil caracteres).
        // Limitamos o tamanho e recuamos até um limite de caractere válido
        // (cortar um "ç" ao meio daria pânico em Rust).
        let mut fim = resto.find(MARCA).unwrap_or(resto.len()).min(40_000);
        while !resto.is_char_boundary(fim) {
            fim -= 1;
        }
        let bloco = &resto[..fim];
        let campo = |marca: &str| {
            bloco.find(marca).map(|p| bloco[p + marca.len()..].split('"').next().unwrap_or("").to_string())
        };
        // "1.661.081 visualizações" → 1661081 (só os dígitos; funciona em qualquer idioma).
        let views = campo(VIEWS)
            .map(|t| t.chars().filter(char::is_ascii_digit).collect::<String>())
            .and_then(|d| d.parse().ok())
            .unwrap_or(0);
        videos.push(Video { id, titulo: campo(TITULO).unwrap_or_default(), views });
        if videos.len() >= 10 {
            break;
        }
    }
    videos
}

/// Só letras e números, minúsculas: "MADEON - Mix | EC123" → "madeonmixec123".
fn normalizar(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Dois títulos (já normalizados) são do mesmo vídeo se começam igual por
/// pelo menos 20 caracteres (ou pelo título inteiro, se for menor).
/// "Contém um no outro" não bastava: o Windows e a página do YouTube às vezes
/// trazem o mesmo título com trechos diferentes no fim.
fn mesmo_titulo(a: &str, b: &str) -> bool {
    let comum = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let menor = a.chars().count().min(b.chars().count());
    menor > 0 && comum >= menor.min(20)
}

/// Entre os 5 primeiros resultados válidos, o MAIS ASSISTIDO.
///
/// Por quê: pegar só o primeiro dava resultados aleatórios (caso real: a busca
/// "Justice mix" abriu um mix de Natal de um canal com 59 inscritos). O mais
/// visto entre os primeiros tende a ser o "canônico": oficial, festival,
/// Boiler Room. Limitamos a 5 para não fugir do que a busca pediu.
///
/// "Válido" = não é o vídeo que está tocando nem um que o Buddy já abriu.
fn escolher_video(html: &str, evitar_titulo: Option<&str>, abertos: &[String]) -> Option<String> {
    let evitar = evitar_titulo.map(normalizar).filter(|t| t.len() >= 8);
    extrair_videos(html)
        .into_iter()
        .filter(|v| {
            let ja_aberto = abertos.contains(&v.id);
            let tocando = evitar.as_deref().is_some_and(|e| mesmo_titulo(e, &normalizar(&v.titulo)));
            !ja_aberto && !tocando
        })
        .take(5)
        // Em empate, max_by_key fica com o ÚLTIMO; com .rev() fica com o primeiro da lista.
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .max_by_key(|v| v.views)
        .map(|v| v.id)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn acao_desconhecida_e_recusada() {
        let json = r#"{ "tipo": "rodar_comando", "comando": "del C:\\" }"#;
        assert!(serde_json::from_str::<Acao>(json).is_err());
    }

    #[test]
    fn consulta_e_codificada_na_url() {
        let buscador = crate::config::Config::default().buscador;
        let url = url_busca_web(&buscador, "a&b=c https://mal.com").unwrap();
        assert!(url.starts_with("https://www.google.com/search?"));
        assert!(url.contains("a%26b%3Dc"));
        let yt = url_busca_youtube("Madeon live set").unwrap();
        assert_eq!(yt, "https://www.youtube.com/results?search_query=Madeon+live+set");
    }

    // Formato igual ao da página real (conferido com curl), bem resumido.
    const HTML: &str = r#"x "videoRenderer":{"videoId":"8Os0Ab1tKzE","t":1,"title":{"runs":[{"text":"MADEON - electric cleaners mix | EC123"}]},"viewCountText":{"simpleText":"5.838 visualizações"}} y "videoRenderer":{"videoId":"dQw4w9WgXcQ","title":{"runs":[{"text":"Madeon - Adventure Live"}]},"viewCountText":{"simpleText":"1.661.081 visualizações"}} z"#;

    #[test]
    fn extrai_ids_e_titulos() {
        let v = extrair_videos(HTML);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], Video { id: "8Os0Ab1tKzE".into(), titulo: "MADEON - electric cleaners mix | EC123".into(), views: 5838 });
        assert_eq!(v[1].views, 1_661_081);
    }

    #[test]
    fn pula_o_video_que_esta_tocando() {
        // Caso real: o título da mídia no Windows é o do vídeo aberto.
        let tocando = "MADEON - electric cleaners mix | electropop & french house | EC123";
        assert_eq!(escolher_video(HTML, Some(tocando), &[]).as_deref(), Some("dQw4w9WgXcQ"));
        // Sem nada tocando: o mais assistido (o 2º da lista, com 1,6 mi).
        assert_eq!(escolher_video(HTML, None, &[]).as_deref(), Some("dQw4w9WgXcQ"));
    }

    #[test]
    fn pula_video_ja_aberto() {
        let abertos = vec!["dQw4w9WgXcQ".to_string()];
        assert_eq!(escolher_video(HTML, None, &abertos).as_deref(), Some("8Os0Ab1tKzE"));
    }

    #[test]
    fn id_estranho_e_recusado() {
        // Tentativa de injetar algo na URL: tem que ser recusado.
        let html = r#""videoRenderer":{"videoId":"abc&evil=1x""#;
        assert_eq!(escolher_video(html, None, &[]), None);
        assert_eq!(escolher_video("<html>sem vídeos</html>", None, &[]), None);
    }

    // Faz uma busca DE VERDADE no YouTube (precisa de internet), por isso
    // #[ignore]. Para rodar:  cargo test youtube -- --ignored --nocapture
    #[test]
    #[ignore]
    fn youtube_de_verdade() {
        tauri::async_runtime::block_on(async {
            for busca in ["Justice mix", "Justice live"] {
                let html = baixar_resultados(busca).await.expect("sem resposta do YouTube");
                println!("
== {busca}");
                for v in extrair_videos(&html).iter().take(5) {
                    println!("  {:>10} views  {}", v.views, v.titulo);
                }
                let escolhido = escolher_video(&html, None, &[]);
                let titulo = extrair_videos(&html).into_iter().find(|v| Some(&v.id) == escolhido.as_ref()).map(|v| v.titulo);
                println!("  -> escolhido: {titulo:?}");
                assert!(escolhido.is_some());
            }
        });
    }
}
