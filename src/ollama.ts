import { invoke } from "@tauri-apps/api/core";

/**
 * O Ollama está respondendo? (LED vermelho / status "Offline" quando não.)
 * Nunca lança erro. Fica num arquivo próprio para a janela de Configurações
 * poder usar sem carregar o resto da IA (zod e companhia).
 */
export async function ollamaNoAr(): Promise<boolean> {
  try {
    return await invoke<boolean>("ollama_ok");
  } catch {
    return false;
  }
}
