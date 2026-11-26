/**
 * Regras de "devo falar agora?".
 *
 * Isto fica no código, e não no modelo, de propósito:
 * - é previsível e testável (o modelo é aleatório);
 * - economiza CPU/GPU: se a regra diz "não", o modelo nem é chamado;
 * - um título de janela malicioso não consegue convencer uma regra em TypeScript.
 *
 * A classe não sabe nada de Tauri, DOM ou modelo: recebe o horário atual como
 * parâmetro. Isso deixa fácil testar ("e se passaram 2 minutos?") sem esperar.
 */

import type { Contexto } from "./contexto";

export interface ConfigRegras {
  /** Mínimo entre duas FALAS automáticas. */
  intervaloEntreFalasMs: number;
  /** Depois de "Agora não", quanto tempo o MESMO assunto fica bloqueado. */
  silencioAposRecusaMs: number;
}

/**
 * Mínimo entre CONSULTAS ao modelo, mesmo quando ele decide não falar.
 * Sem isso, trocar de janela a cada 5s faria o modelo rodar a cada 5s.
 * (Fixo, não está na config: é proteção de desempenho, não preferência.)
 */
export const MIN_ENTRE_CONSULTAS_MS = 30_000;

export class Regras {
  private ultimaFala = -Infinity;
  private ultimaConsulta = -Infinity;
  /** assunto → horário (ms) a partir do qual pode voltar a falar dele. */
  private recusas = new Map<string, number>();
  naoPerturbe = false;

  // `() => ConfigRegras` (uma função) em vez do objeto: assim, quando a config
  // é recarregada, as regras já enxergam os valores novos sem recriar nada.
  private config: () => ConfigRegras;

  constructor(config: () => ConfigRegras) {
    this.config = config;
  }

  /**
   * Devolve `null` se PODE consultar o modelo, ou o motivo de não poder.
   * Só vale para falas automáticas. Clique no Buddy não passa por aqui.
   */
  motivoParaNaoFalar(assunto: string, agora: number): string | null {
    const cfg = this.config();
    if (this.naoPerturbe) return "modo não perturbe";
    if (agora - this.ultimaFala < cfg.intervaloEntreFalasMs) {
      return `falou há ${Math.round((agora - this.ultimaFala) / 1000)}s`;
    }
    if (agora - this.ultimaConsulta < MIN_ENTRE_CONSULTAS_MS) {
      return `consultou o modelo há ${Math.round((agora - this.ultimaConsulta) / 1000)}s`;
    }
    const liberado = this.recusas.get(assunto);
    if (liberado !== undefined && agora < liberado) {
      return `você disse "Agora não" para "${assunto}"`;
    }
    return null;
  }

  registrarConsulta(agora: number): void {
    this.ultimaConsulta = agora;
  }

  registrarFala(agora: number): void {
    this.ultimaFala = agora;
  }

  registrarRecusa(assunto: string, agora: number): void {
    this.recusas.set(assunto, agora + this.config().silencioAposRecusaMs);
    // Limpeza: remove recusas que já venceram, para o Map não crescer para sempre.
    for (const [a, ate] of this.recusas) if (ate <= agora) this.recusas.delete(a);
  }
}

/**
 * "Assunto" de uma conversa: o que identifica "o mesmo tema" para o "Agora não".
 * - Mídia: o ARTISTA (recusou falar de Porter Robinson? Outra faixa dele também
 *   fica bloqueada; um artista diferente, não).
 * - Janela: o PROGRAMA (recusou comentário no VS Code? Fica quieto no VS Code
 *   inteiro, não só naquele arquivo).
 */
export function assuntoDe(c: Contexto, foco: "janela" | "midia" | "tempo"): string {
  // Marco de tempo tem assunto próprio: recusar "pausa?" no VS Code não deve
  // impedir comentários sobre o VS Code, e vice-versa.
  if (foco === "tempo" && c.janela) return `tempo:${c.janela.processo.toLowerCase()}`;
  if (foco === "midia" && c.midia) {
    return `midia:${(c.midia.artista || c.midia.titulo).toLowerCase()}`;
  }
  if (c.janela) return `janela:${c.janela.processo.toLowerCase()}`;
  return "desconhecido";
}
