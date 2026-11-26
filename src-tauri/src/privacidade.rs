//! O que o Buddy NUNCA pode ver, esteja ou não no modo "não perturbe".
//!
//! Por que uma lista embutida no código, além da config?
//! A config é editável; se alguém apagar sem querer a linha `apps_ignorados`,
//! o Buddy voltaria a enxergar o WhatsApp. Esta lista é o PISO: a config só
//! consegue ADICIONAR itens, nunca remover estes.
//!
//! Duas formas de bloquear:
//! - pelo PROGRAMA (.exe): pega o app inteiro (WhatsApp Desktop, Outlook...);
//! - pelo TÍTULO da janela: necessário porque banco, Gmail e WhatsApp Web rodam
//!   DENTRO do navegador, e aí o .exe é só "chrome.exe"/"msedge.exe".
//!
//! O filtro roda no Rust: o título bloqueado nem chega ao TypeScript, muito
//! menos ao modelo de IA.

/// Comparação sem diferenciar maiúsculas/minúsculas.
const APPS: &[&str] = &[
    // Mensagens
    "WhatsApp.exe",
    "WhatsApp.Root.exe",
    "Discord.exe",
    "DiscordPTB.exe",
    "DiscordCanary.exe",
    "Telegram.exe",
    "Signal.exe",
    "Messenger.exe",
    "slack.exe",
    "ms-teams.exe",
    "Teams.exe",
    // E-mail
    "OUTLOOK.EXE",
    "olk.exe",       // Outlook novo
    "HxOutlook.exe", // app Email do Windows
    "thunderbird.exe",
    // Senhas
    "KeePass.exe",
    "KeePassXC.exe",
    "1Password.exe",
    "Bitwarden.exe",
];

/// Trechos de título (em minúsculas). Se o título CONTÉM qualquer um, é privado.
///
/// Cuidado ao adicionar palavras curtas: "rico" bloquearia "histórico", e
/// "banco" bloquearia "banco de dados" (aula de BD!). Por isso só termos bem
/// específicos. Na dúvida, um falso positivo aqui é aceitável: o pior que
/// acontece é o Buddy ficar quieto.
const TITULOS: &[&str] = &[
    // Mensagens na web
    "whatsapp", "discord", "telegram", "messenger", "microsoft teams",
    // E-mail na web
    "gmail", "outlook", "hotmail", "caixa de entrada", "inbox", "proton mail",
    "protonmail", "yahoo mail", "thunderbird",
    // Bancos e pagamentos
    "internet banking", "nubank", "itaú", "itau", "bradesco", "banco do brasil",
    "santander", "caixa econômica", "caixa economica", "caixa tem", "banco inter",
    "c6 bank", "btg pactual", "picpay", "mercado pago", "pagbank", "pagseguro",
    "sicoob", "sicredi", "banrisul", "paypal", "wise.com",
    // Governo / dados pessoais
    "gov.br", "receita federal", "meu inss",
    // Gerenciadores de senha na web
    "bitwarden", "1password", "keepass", "lastpass",
    // Abas anônimas (o navegador põe isso no título)
    "inprivate", "navegação privativa", "navegação anônima", "private browsing", "incognito",
];

/// `true` se a janela não deve ser vista.
pub fn janela_privada(
    processo: &str,
    titulo: &str,
    apps_extra: &[String],
    titulos_extra: &[String],
) -> bool {
    let app_bloqueado = APPS.iter().any(|a| a.eq_ignore_ascii_case(processo))
        || apps_extra.iter().any(|a| a.eq_ignore_ascii_case(processo));
    app_bloqueado || texto_privado(titulo, titulos_extra)
}

/// `true` se o texto contém algum termo bloqueado.
/// Usado também na mídia: um áudio tocando pelo WhatsApp/Discord não é assunto.
pub fn texto_privado(texto: &str, extra: &[String]) -> bool {
    let t = texto.to_lowercase();
    TITULOS.iter().any(|p| t.contains(p))
        || extra.iter().any(|p| !p.trim().is_empty() && t.contains(&p.to_lowercase()))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn bloqueia_apps_e_sites_sensiveis() {
        let nada: &[String] = &[];
        assert!(janela_privada("WhatsApp.exe", "WhatsApp", nada, nada));
        assert!(janela_privada("discord.exe", "#geral - Discord", nada, nada)); // maiúsc./minúsc.
        assert!(janela_privada("msedge.exe", "(3) WhatsApp - Microsoft Edge", nada, nada));
        assert!(janela_privada("chrome.exe", "Caixa de entrada (12) - eu@gmail.com - Gmail", nada, nada));
        assert!(janela_privada("chrome.exe", "Nubank - Google Chrome", nada, nada));
        assert!(janela_privada("msedge.exe", "[InPrivate] Nova guia - Microsoft Edge", nada, nada));
    }

    #[test]
    fn nao_bloqueia_coisas_normais() {
        let nada: &[String] = &[];
        assert!(!janela_privada("Code.exe", "main.rs - buddy - Visual Studio Code", nada, nada));
        assert!(!janela_privada("chrome.exe", "Aula 3: Banco de Dados relacional - YouTube", nada, nada));
        assert!(!janela_privada("msedge.exe", "Histórico de commits - GitHub", nada, nada));
    }

    #[test]
    fn config_so_adiciona() {
        let extra = vec!["Spotify.exe".to_string()];
        let titulos = vec!["Projeto Secreto".to_string()];
        assert!(janela_privada("spotify.exe", "x", &extra, &[]));
        assert!(janela_privada("chrome.exe", "projeto secreto - docs", &[], &titulos));
        // Linha vazia na config não pode bloquear TUDO ("".contains é sempre true)
        assert!(!texto_privado("qualquer coisa", &["  ".to_string()]));
    }
}

/// As listas embutidas, para a janela de Configurações MOSTRAR (só leitura).
#[derive(serde::Serialize)]
pub struct ListasFixas {
    apps: Vec<&'static str>,
    titulos: Vec<&'static str>,
}

#[tauri::command]
pub fn listas_privacidade() -> ListasFixas {
    ListasFixas { apps: APPS.to_vec(), titulos: TITULOS.to_vec() }
}
