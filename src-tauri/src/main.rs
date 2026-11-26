// Em release, esta linha impede que abra uma janela preta de terminal junto com o app.
// Em debug (npm run tauri dev) o terminal continua, o que é útil para ver logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Cada `mod x;` inclui o arquivo src/x.rs.
mod acoes;
mod bandeja;
mod config;
mod contexto;
mod ia;
mod privacidade;

use tauri::Manager;

fn main() {
    // O Rust fica só com o que o navegador não consegue fazer sozinho:
    // ler a janela ativa e a mídia, chamar o Ollama, abrir o navegador,
    // o ícone da bandeja e o arquivo de config.
    // Toda a lógica de "quando falar" fica no TypeScript.
    tauri::Builder::default()
        // `setup` roda uma vez, quando o app inicia.
        .setup(|app| {
            // %APPDATA%\com.buddy.desktop\config.toml
            let caminho = app.path().app_config_dir()?.join("config.toml");
            println!("[buddy] config em {}", caminho.display());
            // `manage` guarda o estado; comandos o recebem via `tauri::State`.
            app.manage(config::EstadoConfig::new(caminho));
            bandeja::criar(app.handle())?;
            Ok(())
        })
        // Registra os comandos que o front pode chamar com invoke("nome").
        .invoke_handler(tauri::generate_handler![
            contexto::get_context,
            ia::ask_llm,
            ia::ollama_ok,
            acoes::executar_acao,
            config::get_config,
            bandeja::get_nao_perturbe,
            bandeja::set_nao_perturbe,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Buddy");
}
