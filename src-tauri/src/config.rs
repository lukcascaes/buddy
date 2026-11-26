//! Arquivo de configuração do Buddy.
//!
//! Fica em  %APPDATA%\com.buddy.desktop\config.toml  (criado na primeira execução).
//! Pode ser editado pela janela de Configurações (bandeja → Configurações…)
//! ou à mão, como opção avançada.
//!
//! Por que o Rust lê E grava o arquivo (e não o TypeScript)?
//! Porque parte da config é usada SÓ no Rust e é sensível: a URL do Ollama e o
//! buscador. O front pode PEDIR para salvar (`salvar_config`), mas tudo passa por
//! `validar` aqui antes de valer. Um bug no front não consegue gravar um valor perigoso.

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
    /// Para onde o "Sim" leva: "youtube_tocar" | "youtube_busca" | "web".
    pub destino_sugestoes: String,
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
            destino_sugestoes: "youtube_tocar".into(),
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

/// Monta o texto do arquivo, COM comentários, a partir dos valores.
///
/// Por que não `toml::to_string(&cfg)`? Ele gera só `chave = valor`, sem os
/// comentários que explicam cada opção. Como a janela de Configurações grava o
/// arquivo, cada salvamento apagaria as explicações. Aqui o "molde" é fixo e só
/// os valores mudam.
///
/// `toml::Value::...to_string()` cuida das aspas e de escapar caracteres
/// especiais (um título com " ou \ não quebra o arquivo).
fn gerar_arquivo(c: &Config) -> String {
    let texto = |s: &str| toml::Value::String(s.to_string()).to_string();
    let lista = |v: &[String]| {
        toml::Value::Array(v.iter().map(|s| toml::Value::String(s.clone())).collect()).to_string()
    };

    format!(
        r#"# Configuração do Buddy
# O jeito fácil de mudar isto é pela janela: ícone da bandeja → Configurações…
# Editando à mão: salve e use "Recarregar do arquivo" na janela de Configurações.
# Se apagar uma linha, o valor padrão é usado.

# --- IA (Ollama) ---
ollama_url = {ollama_url}
# Qualquer modelo baixado com "ollama pull". Ex.: "llama3.2:3b" (mais leve)
modelo = {modelo}

# --- Ação "abrir busca" (o que o "Sim" faz) ---
# "youtube_tocar" = abre o 1º vídeo do YouTube para a sugestão (se falhar, abre a busca)
# "youtube_busca" = abre a busca no YouTube
# "web"           = abre a busca no buscador abaixo
destino_sugestoes = {destino}
# Buscador do modo "web". Precisa ser https. O texto vai no parâmetro ?q=
# Ex.: "https://duckduckgo.com/" ou "https://www.bing.com/search"
buscador = {buscador}

# --- Frequência ---
# Tempo mínimo entre duas falas automáticas (clicar no Buddy ignora isso)
intervalo_entre_falas_seg = {intervalo}
# Depois de "Agora não", não volta ao MESMO assunto por este tempo
silencio_apos_recusa_min = {silencio}
# O contexto precisa ficar parado este tempo antes de ele pensar
estabilizar_seg = {estabilizar}
# De quanto em quanto tempo ele olha a janela ativa e a mídia
leitura_contexto_seg = {leitura}
# Quanto tempo o balão fica na tela (pausa com o mouse em cima)
balao_seg = {balao}

# --- Privacidade ---
# O Buddy JÁ ignora sempre (lista embutida, não dá para remover):
#   WhatsApp, Discord, Telegram, Signal, Slack, Teams, Outlook/Email/Thunderbird,
#   gerenciadores de senha, bancos, Gmail/Hotmail, gov.br e abas anônimas.
# Aqui você só ACRESCENTA. Ex.: apps_ignorados = ["Spotify.exe"]
apps_ignorados = {apps}
# Trechos de título (sem diferenciar maiúsculas). Ex.: ["Projeto Secreto", "Notion"]
titulos_ignorados = {titulos}
"#,
        ollama_url = texto(&c.ollama_url),
        modelo = texto(&c.modelo),
        buscador = texto(&c.buscador),
        destino = texto(&c.destino_sugestoes),
        intervalo = c.intervalo_entre_falas_seg,
        silencio = c.silencio_apos_recusa_min,
        estabilizar = c.estabilizar_seg,
        leitura = c.leitura_contexto_seg,
        balao = c.balao_seg,
        apps = lista(&c.apps_ignorados),
        titulos = lista(&c.titulos_ignorados),
    )
}

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

    fn trocar(&self, nova: Config) {
        if let Ok(mut atual) = self.atual.write() {
            *atual = nova;
        }
    }
}

fn carregar(caminho: &Path) -> Config {
    if !caminho.exists() {
        if let Some(pasta) = caminho.parent() {
            let _ = std::fs::create_dir_all(pasta);
        }
        if let Err(e) = std::fs::write(caminho, gerar_arquivo(&Config::default())) {
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

/// Valores aceitos em `destino_sugestoes`. Qualquer outra coisa vira o padrão.
pub const DESTINOS: [&str; 3] = ["youtube_tocar", "youtube_busca", "web"];

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

    if !DESTINOS.contains(&c.destino_sugestoes.as_str()) {
        c.destino_sugestoes = padrao.destino_sugestoes.clone();
    }

    c.modelo = c.modelo.trim().to_string();
    if c.modelo.is_empty() || c.modelo.len() > 100 {
        c.modelo = padrao.modelo;
    }

    // Limites para ninguém transformar o Buddy num spammer (ou num peso para o PC).
    c.intervalo_entre_falas_seg = c.intervalo_entre_falas_seg.max(30);
    c.silencio_apos_recusa_min = c.silencio_apos_recusa_min.clamp(1, 24 * 60);
    c.leitura_contexto_seg = c.leitura_contexto_seg.clamp(1, 60);
    c.estabilizar_seg = c.estabilizar_seg.min(60);
    c.balao_seg = c.balao_seg.clamp(3, 120);

    // Listas: sem itens vazios (um "" bloquearia TUDO), sem repetidos, com limite.
    c.apps_ignorados = limpar_lista(c.apps_ignorados);
    c.titulos_ignorados = limpar_lista(c.titulos_ignorados);
    c
}

fn limpar_lista(itens: Vec<String>) -> Vec<String> {
    let mut saida: Vec<String> = Vec::new();
    for item in itens {
        let item: String = item.trim().chars().filter(|c| !c.is_control()).take(100).collect();
        let repetido = saida.iter().any(|s| s.eq_ignore_ascii_case(&item));
        if !item.is_empty() && !repetido && saida.len() < 100 {
            saida.push(item);
        }
    }
    saida
}

/// Relê o arquivo e avisa todas as janelas (evento "config-alterada").
pub fn recarregar(app: &AppHandle) -> Config {
    let estado = app.state::<EstadoConfig>();
    let nova = carregar(&estado.caminho);
    estado.trocar(nova.clone());
    println!("[buddy] configuração recarregada: {nova:?}");
    let _ = app.emit("config-alterada", nova.clone());
    nova
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

/// Chamado pela janela de Configurações. Valida, grava o arquivo, troca a config
/// em uso e avisa todas as janelas. Devolve a config COMO FICOU (o front mostra
/// o valor corrigido, se `validar` mudou algo).
#[tauri::command]
pub fn salvar_config(app: AppHandle, nova: Config) -> Result<Config, String> {
    let estado = app.state::<EstadoConfig>();
    let nova = validar(nova);
    std::fs::write(&estado.caminho, gerar_arquivo(&nova))
        .map_err(|e| format!("não consegui gravar a configuração: {e}"))?;
    estado.trocar(nova.clone());
    let _ = app.emit("config-alterada", nova.clone());
    Ok(nova)
}

#[tauri::command]
pub fn recarregar_config(app: AppHandle) -> Config {
    recarregar(&app)
}

#[tauri::command]
pub fn abrir_arquivo_config(app: AppHandle) {
    abrir_no_editor(&app);
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn arquivo_gerado_volta_igual() {
        // Grava e lê de volta: tem que dar exatamente a mesma config.
        let padrao = Config::default();
        assert_eq!(toml::from_str::<Config>(&gerar_arquivo(&padrao)).unwrap(), padrao);

        let diferente = Config {
            modelo: "llama3.2:3b".into(),
            apps_ignorados: vec!["Spotify.exe".into()],
            // Aspas e barra invertida não podem quebrar o arquivo:
            titulos_ignorados: vec![r#"Projeto "Secreto" \ v2"#.into()],
            intervalo_entre_falas_seg: 600,
            ..Config::default()
        };
        assert_eq!(toml::from_str::<Config>(&gerar_arquivo(&diferente)).unwrap(), diferente);
    }

    #[test]
    fn arquivo_gerado_mantem_comentarios() {
        assert!(gerar_arquivo(&Config::default()).contains("# --- Privacidade ---"));
    }

    #[test]
    fn valores_perigosos_sao_corrigidos() {
        let c = validar(Config {
            buscador: "file:///C:/Windows/System32/calc.exe".into(),
            ollama_url: "javascript:alert(1)".into(),
            destino_sugestoes: "rodar_programa".into(),
            intervalo_entre_falas_seg: 1,
            ..Config::default()
        });
        assert_eq!(c.buscador, Config::default().buscador);
        assert_eq!(c.ollama_url, Config::default().ollama_url);
        assert_eq!(c.intervalo_entre_falas_seg, 30);
        assert_eq!(c.destino_sugestoes, "youtube_tocar");
    }

    #[test]
    fn listas_sao_limpas() {
        let c = validar(Config {
            apps_ignorados: vec!["  ".into(), "Spotify.exe".into(), "spotify.EXE".into()],
            ..Config::default()
        });
        assert_eq!(c.apps_ignorados, vec!["Spotify.exe".to_string()]);
    }

    #[test]
    fn campos_faltando_usam_padrao() {
        let c: Config = toml::from_str("modelo = \"llama3.2:3b\"").unwrap();
        assert_eq!(c.modelo, "llama3.2:3b");
        assert_eq!(c.intervalo_entre_falas_seg, 180);
    }
}
