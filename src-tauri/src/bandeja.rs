//! Ícone na bandeja do Windows (perto do relógio), com menu:
//!   ☐ Não perturbe
//!   Abrir configuração / Recarregar configuração
//!   Sair
//!
//! O "Não perturbe" mora aqui no Rust (um AtomicBool) porque o menu é do Rust.
//! Ele pode mudar de dois jeitos: pelo menu ou pelo duplo clique no Buddy
//! (comando `set_nao_perturbe`). Os dois passam por `aplicar`, que atualiza o
//! menu e avisa o front com o evento "nao-perturbe". Uma fonte de verdade só.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::config;

/// `AtomicBool` = um bool que pode ser lido/escrito por várias threads
/// sem precisar de lock. Perfeito para uma chave liga/desliga.
static NAO_PERTURBE: AtomicBool = AtomicBool::new(false);

/// Guardamos o item do menu para poder marcá-lo/desmarcá-lo quando a mudança
/// vem do duplo clique (e não de um clique no próprio menu).
struct ItemNaoPerturbe(CheckMenuItem<Wry>);

pub fn criar(app: &AppHandle) -> tauri::Result<()> {
    let nao_perturbe =
        CheckMenuItem::with_id(app, "nao_perturbe", "Não perturbe", true, false, None::<&str>)?;
    let abrir = MenuItem::with_id(app, "abrir_config", "Abrir configuração", true, None::<&str>)?;
    let recarregar =
        MenuItem::with_id(app, "recarregar_config", "Recarregar configuração", true, None::<&str>)?;
    let sair = MenuItem::with_id(app, "sair", "Sair", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &nao_perturbe,
            &PredefinedMenuItem::separator(app)?,
            &abrir,
            &recarregar,
            &PredefinedMenuItem::separator(app)?,
            &sair,
        ],
    )?;

    app.manage(ItemNaoPerturbe(nao_perturbe.clone()));

    let mut construtor = TrayIconBuilder::with_id("buddy").tooltip("Buddy").menu(&menu);
    if let Some(icone) = app.default_window_icon() {
        construtor = construtor.icon(icone.clone());
    }

    construtor
        .on_menu_event(move |app, evento| match evento.id().as_ref() {
            "nao_perturbe" => {
                // O Windows já marcou/desmarcou o item; só lemos o novo estado.
                let ligado = app
                    .state::<ItemNaoPerturbe>()
                    .0
                    .is_checked()
                    .unwrap_or(false);
                aplicar(app, ligado);
            }
            "abrir_config" => config::abrir_no_editor(app),
            "recarregar_config" => config::recarregar(app),
            "sair" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn aplicar(app: &AppHandle, ligado: bool) {
    NAO_PERTURBE.store(ligado, Ordering::Relaxed);
    if let Some(item) = app.try_state::<ItemNaoPerturbe>() {
        let _ = item.0.set_checked(ligado);
    }
    let _ = app.emit("nao-perturbe", ligado);
}

#[tauri::command]
pub fn get_nao_perturbe() -> bool {
    NAO_PERTURBE.load(Ordering::Relaxed)
}

/// Chamado pelo duplo clique no Buddy.
#[tauri::command]
pub fn set_nao_perturbe(app: AppHandle, ligado: bool) {
    aplicar(&app, ligado);
}
