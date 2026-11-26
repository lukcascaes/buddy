//! Ações que o Buddy pode executar: uma ALLOWLIST FECHADA.
//!
//! Por que validar aqui de novo, se o TypeScript já valida com zod?
//! Defesa em profundidade: se um dia houver um bug no front (ou alguém injetar
//! JavaScript nele), o máximo que dá para pedir ao Rust é uma das ações deste
//! `enum`. Não existe "abrir URL qualquer" nem "rodar comando": a URL de busca
//! é montada AQUI, e o texto do usuário/modelo só entra como parâmetro `q`.

use serde::Deserialize;

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

/// Comando chamado pelo front SOMENTE depois do clique em "Sim".
#[tauri::command]
pub fn executar_acao(estado: tauri::State<'_, EstadoConfig>, acao: Acao) -> Result<(), String> {
    // O buscador vem da config, que já foi validada como https em config.rs.
    let buscador = estado.ler().buscador;
    match acao {
        Acao::AbrirBusca { consulta } => {
            let consulta: String = consulta
                .chars()
                .filter(|c| !c.is_control())
                .take(MAX_CONSULTA)
                .collect();
            let consulta = consulta.trim();
            if consulta.is_empty() {
                return Err("consulta vazia".into());
            }

            // parse_with_params codifica o texto: "Fred again.." → "Fred+again..",
            // "&" vira "%26" etc. Assim a consulta nunca "escapa" do parâmetro q.
            let url = url::Url::parse_with_params(&buscador, &[("q", consulta)])
                .map_err(|e| e.to_string())?;

            // Garantia extra: só abrimos https (nada de file:, javascript:, etc.).
            if url.scheme() != "https" {
                return Err("URL de busca inválida".into());
            }

            open::that_detached(url.as_str()).map_err(|e| format!("não consegui abrir o navegador: {e}"))
        }
    }
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
        let url = url::Url::parse_with_params(&buscador, &[("q", "a&b=c https://mal.com")]).unwrap();
        assert_eq!(url.host_str(), Some("www.google.com"));
        assert!(url.as_str().contains("a%26b%3Dc"));
    }
}
