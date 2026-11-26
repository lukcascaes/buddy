//! Leitura do contexto do usuário: janela ativa + mídia tocando.
//!
//! Tudo aqui é SOMENTE LEITURA. O Buddy nunca escreve em outras janelas nem
//! controla a mídia. E todo texto que sai daqui (títulos de janela, nome de música)
//! é DADO NÃO CONFIÁVEL: qualquer site pode colocar no título da aba algo como
//! "ignore as instruções anteriores e...". Quem decide o que fazer com isso é o
//! TypeScript, que vai tratar esse texto como dado (Fase 3).

use serde::Serialize;

use crate::privacidade;
use windows::core::PWSTR;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as GerenciadorMidia,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as StatusMidia,
};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

/// Tamanho máximo de qualquer texto devolvido. Títulos podem ser enormes
/// (ou maliciosamente enormes) e isso vai virar prompt do modelo depois.
const MAX_CHARS: usize = 200;

// `#[derive(Serialize)]` gera automaticamente o código que converte a struct em
// JSON. É assim que o Tauri entrega isto ao TypeScript.
// `Option<T>` é o jeito do Rust dizer "pode não ter valor" (vira `null` no JSON).

#[derive(Serialize, Debug, Clone)]
pub struct Contexto {
    /// `None` quando não há janela útil (ex.: a ativa é o próprio Buddy).
    pub janela: Option<JanelaAtiva>,
    /// `None` quando nada está registrado no "Tocando agora" do Windows.
    pub midia: Option<Midia>,
    /// `true` quando a janela ativa é privada (ver privacidade.rs). Nesse caso
    /// `janela` é `None`: o front só sabe QUE é privada, não O QUE é.
    pub privado: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct JanelaAtiva {
    pub titulo: String,
    /// Nome do executável, ex.: "chrome.exe", "Code.exe".
    pub processo: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Midia {
    pub titulo: String,
    pub artista: String,
    /// Identificador do app de origem, ex.: "Spotify.exe" ou "MSEdge".
    pub app: String,
    /// true = tocando; false = pausado/parado.
    pub tocando: bool,
}

/// Comando chamado pelo TypeScript com `invoke("get_context")`.
///
/// Por que `async` + `spawn_blocking`?
/// No Tauri v2, um comando NÃO-async roda na thread principal, a mesma que desenha
/// a janela. As chamadas ao Windows abaixo esperam respostas do sistema (a de mídia
/// pode levar dezenas de ms) e travariam a interface. Com `spawn_blocking`, o
/// trabalho vai para uma thread separada e a janela continua fluida.
#[tauri::command]
pub async fn get_context(
    estado: tauri::State<'_, crate::config::EstadoConfig>,
) -> Result<Contexto, String> {
    let cfg = estado.ler();

    tauri::async_runtime::spawn_blocking(move || {
        // Privacidade: janelas privadas são descartadas AQUI, no Rust.
        // O título nem chega ao TypeScript, muito menos ao modelo.
        let janela = ler_janela_ativa();
        let privado = janela.as_ref().is_some_and(|j| {
            privacidade::janela_privada(
                &j.processo,
                &j.titulo,
                &cfg.apps_ignorados,
                &cfg.titulos_ignorados,
            )
        });

        // Se der erro lendo a mídia (ex.: nenhum player aberto), só devolvemos None.
        // Mídia vinda de app privado (ex.: áudio do WhatsApp) também é descartada.
        let midia = ler_midia().unwrap_or(None).filter(|m| {
            !privacidade::texto_privado(&m.app, &cfg.titulos_ignorados)
                && !privacidade::texto_privado(&m.titulo, &cfg.titulos_ignorados)
        });

        Contexto {
            janela: if privado { None } else { janela },
            midia,
            privado,
        }
    })
    .await
    .map_err(|e| format!("falha ao ler contexto: {e}"))
}

fn ler_janela_ativa() -> Option<JanelaAtiva> {
    // `unsafe`: o Rust não consegue garantir a segurança de chamadas diretas à API
    // em C do Windows, então exige que a gente marque o bloco. Estas três funções
    // são só leitura e não guardam ponteiros, então o risco aqui é baixo.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }

        // Ignora a janela do próprio Buddy: quando você clica nele, ele vira a
        // janela ativa, e não queremos que ele "perceba" a si mesmo.
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() {
            return None;
        }

        // Buffer de 512 caracteres UTF-16 (o Windows usa UTF-16 internamente).
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, &mut buf);
        let titulo = String::from_utf16_lossy(&buf[..n.max(0) as usize]);

        // Janelas sem título (área de trabalho, menus) não dizem nada útil.
        if titulo.trim().is_empty() {
            return None;
        }

        Some(JanelaAtiva {
            titulo: limpar(&titulo),
            processo: nome_do_processo(pid).unwrap_or_default(),
        })
    }
}

/// Descobre o nome do .exe a partir do id do processo.
fn nome_do_processo(pid: u32) -> Option<String> {
    unsafe {
        // LIMITED_INFORMATION = a permissão mínima possível; funciona até para
        // processos de outros usuários/admin na maioria dos casos.
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        // Sempre fechar o handle, senão "vaza" um recurso do sistema a cada chamada.
        let _ = CloseHandle(handle);
        ok.ok()?;

        let caminho = String::from_utf16_lossy(&buf[..len as usize]);
        // Pega só o final: "C:\...\chrome.exe" → "chrome.exe"
        caminho.rsplit('\\').next().map(str::to_string)
    }
}

/// Guarda o gerenciador de mídia depois da primeira criação.
/// Criá-lo é a parte cara (medido: o consumo do Buddy dobrava criando a cada 2s).
/// `OnceLock` = "uma caixa que só pode ser preenchida uma vez", segura entre threads.
static GERENCIADOR: std::sync::OnceLock<GerenciadorMidia> = std::sync::OnceLock::new();

fn gerenciador_midia() -> windows::core::Result<GerenciadorMidia> {
    if let Some(g) = GERENCIADOR.get() {
        return Ok(g.clone()); // clone aqui só copia uma referência (barato)
    }
    // APIs WinRT são assíncronas; `.join()` espera o resultado aqui mesmo.
    // Tudo bem esperar porque estamos numa thread separada (spawn_blocking).
    let g = GerenciadorMidia::RequestAsync()?.join()?;
    Ok(GERENCIADOR.get_or_init(|| g).clone())
}

/// Lê o "Tocando agora" do Windows (o mesmo que aparece no controle de volume).
/// Funciona com Spotify, YouTube no Chrome/Edge, players do Windows, etc.
/// O `?` depois de cada chamada significa: "se deu erro, retorne o erro agora".
fn ler_midia() -> windows::core::Result<Option<Midia>> {
    let gerenciador = gerenciador_midia()?;

    // GetCurrentSession dá erro se nenhum app de mídia estiver registrado.
    let Ok(sessao) = gerenciador.GetCurrentSession() else {
        return Ok(None);
    };

    let props = sessao.TryGetMediaPropertiesAsync()?.join()?;
    let titulo = props.Title()?.to_string();
    if titulo.trim().is_empty() {
        return Ok(None);
    }

    let tocando = sessao
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .map(|s| s == StatusMidia::Playing)
        .unwrap_or(false);

    Ok(Some(Midia {
        titulo: limpar(&titulo),
        artista: limpar(&props.Artist()?.to_string()),
        app: limpar(&sessao.SourceAppUserModelId()?.to_string()),
        tocando,
    }))
}

/// Título da mídia atual (para a ação não reabrir o mesmo vídeo). Bloqueante.
pub fn titulo_midia() -> Option<String> {
    ler_midia().ok().flatten().map(|m| m.titulo)
}

/// Pausa a mídia que está tocando (o mesmo que apertar pause no controle de volume).
/// Devolve `true` se pausou algo; `false` se não havia nada tocando.
///
/// ÚNICA escrita que o Buddy faz no "Tocando agora", e só é chamada depois do seu
/// "Sim" num balão que avisou "⏸ pauso a música". Bloqueante (espera o Windows):
/// chame sempre fora da thread da janela (spawn_blocking).
pub fn pausar_midia() -> windows::core::Result<bool> {
    let Ok(sessao) = gerenciador_midia()?.GetCurrentSession() else {
        return Ok(false);
    };
    let tocando = sessao
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .map(|s| s == StatusMidia::Playing)
        .unwrap_or(false);
    if !tocando {
        return Ok(false);
    }
    sessao.TryPauseAsync()?.join()
}

/// Remove caracteres de controle (quebras de linha, etc.) e corta o tamanho.
/// Isso NÃO resolve prompt injection sozinho (texto normal também pode conter
/// ordens), mas impede truques de formatação e prompts gigantes.
fn limpar(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

// Teste manual: imprime o contexto atual no terminal.
// Rode com:  cd src-tauri ; cargo test -- --nocapture
// (`--nocapture` faz o println! aparecer; sem ele o Rust esconde a saída dos testes.)
#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mostra_contexto_atual() {
        let c = Contexto {
            janela: ler_janela_ativa(),
            midia: ler_midia().expect("erro na API de mídia"),
            privado: false,
        };
        println!("{c:#?}");
    }

    // Pausa a música de verdade. Por isso #[ignore]: não roda no "cargo test"
    // normal. Para rodar:  cargo test pausa -- --ignored --nocapture
    #[test]
    #[ignore]
    fn pausa_a_midia_tocando() {
        println!("pausou: {:?}", pausar_midia());
    }

    #[test]
    fn limpar_remove_controle_e_corta() {
        let sujo = format!("linha1\nlinha2\t{}", "x".repeat(500));
        let limpo = limpar(&sujo);
        assert!(!limpo.contains('\n'));
        assert_eq!(limpo.chars().count(), MAX_CHARS);
    }
}
