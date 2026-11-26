/**
 * Ponte com os comandos Rust de IA e de ações.
 * Aqui não há regra de "quando falar" (isso fica no main.ts / Fase 4):
 * só "pergunte ao modelo e me devolva algo VALIDADO".
 */

import { invoke } from "@tauri-apps/api/core";
import type { Contexto } from "../contexto";
import type { ResumoMemoria } from "../memoria";
import { formatoOllama, interpretarResposta, type Acao, type RespostaModelo } from "./esquema";
import { montarPrompt, SISTEMA, type Mudanca } from "./prompt";

export { ollamaNoAr } from "../ollama";

/**
 * Pergunta ao modelo o que dizer sobre o contexto.
 * - Erro de rede/Ollama → lança exceção (quem chamou decide se avisa o usuário).
 * - Resposta fora do formato → devolve null (ignorada em silêncio, com log).
 */
export async function pensar(
  c: Contexto,
  mudanca: Mudanca,
  memoria: ResumoMemoria,
): Promise<RespostaModelo | null> {
  const inicio = performance.now();
  const texto = await invoke<string>("ask_llm", {
    sistema: SISTEMA,
    prompt: montarPrompt(c, mudanca, memoria),
    formato: formatoOllama,
  });
  console.log(`[buddy] modelo respondeu em ${Math.round(performance.now() - inicio)} ms:`, texto);
  return interpretarResposta(texto);
}

/** Executa uma ação. O Rust valida de novo (allowlist dupla). */
export function executarAcao(acao: Acao, pausar: boolean): Promise<void> {
  return invoke("executar_acao", { acao, pausar });
}

