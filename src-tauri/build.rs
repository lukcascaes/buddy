// Script que roda ANTES de compilar o app. O Tauri usa ele para gerar código a partir
// do tauri.conf.json e das capabilities (permissões), e para embutir o ícone no .exe.
fn main() {
    // O tauri-build só pede para rodar de novo quando o tauri.conf.json muda.
    // Trocar os arquivos de ícone NÃO disparava nada, e o app seguia com o
    // ícone antigo (foi assim que o Buddy roxo continuou na bandeja).
    // Esta linha manda o Cargo rodar este script também quando a pasta icons/ mudar.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
