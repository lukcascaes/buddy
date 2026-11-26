import { defineConfig } from "vite";

// Configuração recomendada pelo Tauri para o Vite.
export default defineConfig({
  // Não limpa o terminal: assim os erros do Rust (cargo) continuam visíveis.
  clearScreen: false,
  server: {
    // O Tauri espera o front exatamente nesta porta (ver devUrl no tauri.conf.json).
    // strictPort: se a porta estiver ocupada, falha em vez de "pular" para outra.
    port: 1420,
    strictPort: true,
    // Não recarrega o front quando arquivos do Rust mudam (o Tauri cuida disso).
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // O WebView2 do Windows é Chromium moderno, então podemos gerar JS moderno.
    target: "chrome105",
    // Duas páginas: o Buddy (index.html) e a janela de Configurações.
    // Sem isto, o build só incluiria o index.html.
    rollupOptions: {
      input: { buddy: "index.html", config: "config.html" },
    },
  },
});
