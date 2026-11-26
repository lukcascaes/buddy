//! Arquivo de configuração do Buddy.
//!
//! Fica em  %APPDATA%\com.buddy.desktop\config.toml  (criado na primeira execução).
//! Pode ser aberto e recarregado pelo ícone da bandeja, sem reiniciar o app.
//!
//! Por que o Rust lê o arquivo (e não o TypeScript)?
//! Porque parte da config é usada SÓ no Rust e é sensível: a URL do Ollama e o
//! buscador. Se viessem do front, um bug no front poderia mandar o Rust acessar
//! qualquer endereço. Assim o front só LÊ a config, nunca a define.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tauri::{AppHandle, Emitter, Manager};

/// `#[serde(default)]`: campos que faltarem no arquivo usam o valor de `Default`.
/// Assim um arquivo antigo (ou com uma linha apagada) continua funcionando.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub ollama_url: String,
    pub modelo: String,
    pub buscador: String,
    pub intervalo_entre_falas_seg: u64,
    pub silencio_apos_recusa_min: u64,
    pub estabilizar_seg: u64,
    pub leitura_contexto_seg: u64,
    pub balao_seg: u64,
    /// EXTRAS: somados à lista embutida em privacidade.rs (que não sai).
    pub apps_ignorados: Vec<String>,
    pub titulos_ignorados: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ollama_url: "http://localhost:11434".into(),
            modelo: "qwen2.5:7b".into(),
            buscador: "https://www.google.com/search".into(),
            intervalo_entre_falas_seg: 180,
            silencio_apos_recusa_min: 60,
            estabilizar_seg: 4,
            leitura_contexto_seg: 2,
            balao_seg: 12,
            apps_ignorados: vec![],
            titulos_ignorados: vec![],
        }
    }
}

/// Arquivo criado na primeira execução. Os valores TÊM que bater com o
/// `Default` acima (há um teste lá embaixo que garante isso).
const ARQUIVO_PADRAO: &str = r#"# Configuração do Buddy
# Depois de editar, salve e use "Recarregar configuração" no ícone da bandeja.
# Se apagar uma linha, o valor padrão é usado.

# --- IA (Ollama) ---
ollama_url = "http://localhost:11434"
# Qualquer modelo baixado com "ollama pull". Ex.: "llama3.2:3b" (mais leve)
modelo = "qwen2.5:7b"

# --- Ação "abrir busca" ---
# Precisa ser https. O texto vai no parâmetro ?q=
# Ex.: "https://duckduckgo.com/" ou "https://www.bing.com/search"
buscador = "https://www.google.com/search"

# --- Frequência ---
# Tempo mínimo entre duas falas automáticas (clicar no Buddy ignora isso)
intervalo_entre_falas_seg = 180
# Depois de "Agora não", não volta ao MESMO assunto por este tempo
silencio_apos_recusa_min = 60
# O contexto precisa ficar parado este tempo antes de ele pensar
estabilizar_seg = 4
# De quanto em quanto tempo ele olha a janela ativa e a mídia
leitura_contexto_seg = 2
# Quanto tempo o balão fica na tela (pausa com o mouse em cima)
balao_seg = 12

# --- Privacidade ---
# O Buddy JÁ ignora sempre (lista embutida, não dá para remover):
#   WhatsApp, Discord, Telegram, Signal, Slack, Teams, Outlook/Email/Thunderbird,
#   gerenciadores de senha, bancos, Gmail/Hotmail, gov.br e abas anônimas.
# Aqui você só ACRESCENTA. Ex.: apps_ignorados = ["Spotify.exe"]
apps_ignorados = []
# Trechos de título (sem diferenciar maiúsculas). Ex.: ["Projeto Secreto", "Notion"]
titulos_ignorados = []
"#;

/// Guardado no Tauri com `app.manage(...)` e acessado nos comandos via `State`.
/// `RwLock` = vários podem LER ao mesmo tempo; para ESCREVER (recarregar),
/// espera todo mundo sair. Necessário porque comandos rodam em várias threads.
pub struct EstadoConfig {
    pub caminho: PathBuf,
    atual: RwLock<Config>,
}

impl EstadoConfig {
    pub fn new(caminho: PathBuf) -> Self {
        let cfg = carregar(&caminho);
        Self { caminho, atual: RwLock::new(cfg) }
    }

    /// Cópia da config atual. Copiar (clone) evita segurar o lock por muito
    /// tempo, por exemplo durante uma chamada ao Ollama de vários segundos.
    pub fn ler(&self) -> Config {
        self.atual.read().map(|c| c.clone()).unwrap_or_default()
    }
}

fn carregar(caminho: &Path) -> Config {
    if !caminho.exists() {
        if let Some(pasta) = caminho.parent() {
            let _ = std::fs::create_dir_all(pasta);
        }
        if let Err(e) = std::fs::write(caminho, ARQUIVO_PADRAO) {
            eprintln!("[buddy] não consegui criar {}: {e}", caminho.display());
        }
    }

    let cfg = match std::fs::read_to_string(caminho) {
        Ok(texto) => toml::from_str::<Config>(&texto).unwrap_or_else(|e| {
            // Arquivo com erro de sintaxe: avisa e segue com o padrão,
            // em vez de o app nem abrir.
            eprintln!("[buddy] config inválida, usando padrão: {e}");
            Config::default()
        }),
        Err(_) => Config::default(),
    };
    validar(cfg)
}

/// Corrige valores absurdos ou perigosos.
fn validar(mut c: Config) -> Config {
    let padrao = Config::default();

    // URLs: só aceitamos o que faz sentido para cada uma.
    let ollama_ok = url::Url::parse(&c.ollama_url)
        .map(|u| u.scheme() == "http" || u.scheme() == "https")
        .unwrap_or(false);
    if !ollama_ok {
        eprintln!("[buddy] ollama_url inválida, usando padrão");
        c.ollama_url = padrao.ollama_url.clone();
    }
    c.ollama_url = c.ollama_url.trim_end_matches('/').to_string();

    let buscador_ok = url::Url::parse(&c.buscador).map(|u| u.scheme() == "https").unwrap_or(false);
    if !buscador_ok {
        eprintln!("[buddy] buscador precisa ser https, usando padrão");
        c.buscador = padrao.buscador.clone();
    }

    if c.modelo.trim().is_empty() {
        c.modelo = padrao.modelo;
    }

    // Limites para ninguém transformar o Buddy num spammer (ou num peso para o PC).
    c.intervalo_entre_falas_seg = c.intervalo_entre_falas_seg.max(30);
    c.leitura_contexto_seg = c.leitura_contexto_seg.clamp(1, 60);
    c.estabilizar_seg = c.estabilizar_seg.min(60);
    c.balao_seg = c.balao_seg.clamp(3, 120);
    c
}

/// Relê o arquivo e avisa o front (evento "config-alterada").
pub fn recarregar(app: &AppHandle) {
    let estado = app.state::<EstadoConfig>();
    let nova = carregar(&estado.caminho);
    if let Ok(mut atual) = estado.atual.write() {
        *atual = nova.clone();
    }
    println!("[buddy] configuração recarregada: {nova:?}");
    let _ = app.emit("config-alterada", nova);
}

/// Abre o arquivo no programa que o Windows associa a .toml (no seu caso, VS Code).
///
/// Histórico: a primeira versão chamava `notepad.exe` direto. O Notepad novo do
/// Windows 11 (versão da Store) repassa o arquivo para uma janela já aberta e,
/// nessa passagem, dá "O sistema não pode encontrar o caminho especificado",
/// mesmo com o caminho certo. Lição: não dependa de um app específico.
///
/// Plano B: se não houver programa associado, abre o Explorer com o arquivo
/// selecionado, e você escolhe com o que abrir.
pub fn abrir_no_editor(app: &AppHandle) {
    let caminho = app.state::<EstadoConfig>().caminho.clone();
    if let Err(e) = open::that_detached(&caminho) {
        eprintln!("[buddy] sem programa para .toml ({e}); abrindo a pasta");
        // `Command` sem shell: o caminho vai como argumento, sem risco de injeção.
        let _ = std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", caminho.display()))
            .spawn();
    }
}

#[tauri::command]
pub fn get_config(estado: tauri::State<'_, EstadoConfig>) -> Config {
    estado.ler()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn arquivo_padrao_bate_com_default() {
        let lido: Config = toml::from_str(ARQUIVO_PADRAO).unwrap();
        assert_eq!(lido, Config::default());
    }

    #[test]
    fn valores_perigosos_sao_corrigidos() {
        let c = validar(Config {
            buscador: "file:///C:/Windows/System32/calc.exe".into(),
            ollama_url: "javascript:alert(1)".into(),
            intervalo_entre_falas_seg: 1,
            ..Config::default()
        });
        assert_eq!(c.buscador, Config::default().buscador);
        assert_eq!(c.ollama_url, Config::default().ollama_url);
        assert_eq!(c.intervalo_entre_falas_seg, 30);
    }

    #[test]
    fn campos_faltando_usam_padrao() {
        let c: Config = toml::from_str("modelo = \"llama3.2:3b\"").unwrap();
        assert_eq!(c.modelo, "llama3.2:3b");
        assert_eq!(c.intervalo_entre_falas_seg, 180);
    }
}
