/**
 * Memória curta do Buddy: o que você fez nos últimos minutos e o que ele já disse.
 *
 * Por que isso deixa o modelo "mais esperto"?
 * Antes ele recebia só uma FOTO ("janela X, música Y"). Com uma foto, o melhor
 * que dá para dizer é o óbvio ("mexendo no VS Code, hein?"). Com um pequeno
 * FILME ("2h no mesmo app, 4 faixas seguidas do mesmo artista, 1h da manhã,
 * e você já recusou uma sugestão parecida"), ele tem assunto de verdade.
 *
 * Regras de privacidade desta memória:
 * - fica só na RAM (some ao fechar o app; nada vai para disco);
 * - janelas privadas não entram (o Rust já entrega sem título);
 * - ao modelo vão só NOMES DE PROGRAMA e minutos, não a lista de títulos.
 */

import type { Contexto } from "./contexto";

/** Um período contínuo na mesma janela. */
interface Trecho {
  processo: string;
  titulo: string;
  inicio: number;
  fim: number;
}

interface Faixa {
  artista: string;
  titulo: string;
  inicio: number;
}

export type Reacao = "aceitou" | "recusou" | "ignorou" | "so_falou";

interface FalaLembrada {
  quando: number;
  texto: string;
  /** Texto da busca oferecida, se houve ação. */
  ofereceu?: string;
  reacao: Reacao | null;
}

/** O que vai para o prompt (tudo pequeno e já mastigado). */
export interface ResumoMemoria {
  agora: { hora: string; dia: string; periodo: string };
  /** Minutos na janela atual (mesmo título). */
  min_nesta_janela?: number;
  /** Minutos seguidos no mesmo PROGRAMA (trocando de aba/arquivo dentro dele). */
  min_neste_app?: number;
  /** Quantas faixas seguidas do artista atual. */
  faixas_seguidas_do_artista?: number;
  /** Programas mais usados nos últimos 30 min. */
  ultimos_30_min: { app: string; min: number }[];
  /** Últimas faixas, da mais recente para a mais antiga. */
  faixas_recentes: string[];
  /** O que o Buddy disse recentemente e como você reagiu. */
  suas_falas_recentes: { ha_min: number; fala: string; ofereceu?: string; reacao: Reacao | null }[];
}

/**
 * Palavras que não identificam o QUE foi sugerido ("Madeon live set" e
 * "Madeon mix" são a mesma sugestão: o que importa é "madeon").
 */
const PALAVRAS_GENERICAS = new Set([
  "live", "set", "mix", "dj", "playlist", "ao", "vivo", "musica", "musicas", "música",
  "músicas", "de", "do", "da", "e", "the", "best", "of", "songs", "album", "álbum",
]);

function termos(consulta: string): Set<string> {
  return new Set(
    consulta
      .toLowerCase()
      .split(/[^\p{L}\p{N}]+/u) // quebra em tudo que não for letra/número
      .filter((p) => p.length > 1 && !PALAVRAS_GENERICAS.has(p)),
  );
}

// 5h de histórico: precisa cobrir o maior marco de tempo (240 min) com folga.
// Antes era 1h, e uma sessão longa "perdia o começo" (pego no teste).
const GUARDAR_MS = 5 * 60 * 60_000;
const MAX_TRECHOS = 2000; // teto de segurança (cada trecho é ~100 bytes)
/** Um "buraco" maior que isto entre leituras quebra a sequência (PC suspenso etc.). */
const BURACO_MS = 2 * 60_000;
const MAX_FAIXAS = 20;
const MAX_FALAS = 8;

export class Memoria {
  private trechos: Trecho[] = [];
  private faixas: Faixa[] = [];
  private falas: FalaLembrada[] = [];

  /** Chamado a CADA leitura de contexto (não só quando muda). */
  registrarContexto(c: Contexto, agora: number): void {
    this.registrarJanela(c, agora);
    this.registrarFaixa(c, agora);
  }

  private registrarJanela(c: Contexto, agora: number): void {
    const ultimo = this.trechos.at(-1);
    if (c.privado || !c.janela) return; // nada entra; o buraco quebra a sequência

    const mesma =
      ultimo &&
      ultimo.processo === c.janela.processo &&
      ultimo.titulo === c.janela.titulo &&
      agora - ultimo.fim < BURACO_MS;

    if (mesma) {
      ultimo.fim = agora;
    } else {
      this.trechos.push({ processo: c.janela.processo, titulo: c.janela.titulo, inicio: agora, fim: agora });
    }
    // Esquece o que for mais velho que 5h (e nunca guarda mais que MAX_TRECHOS).
    this.trechos = this.trechos.filter((t) => agora - t.fim < GUARDAR_MS).slice(-MAX_TRECHOS);
  }

  private registrarFaixa(c: Contexto, agora: number): void {
    const m = c.midia;
    if (!m || !m.tocando) return;
    const ultima = this.faixas.at(-1);
    if (ultima && ultima.artista === m.artista && ultima.titulo === m.titulo) return;
    this.faixas.push({ artista: m.artista, titulo: m.titulo, inicio: agora });
    if (this.faixas.length > MAX_FAIXAS) this.faixas.shift();
  }

  /** Guarda uma fala do Buddy. Devolve um id para registrar a reação depois. */
  registrarFala(texto: string, agora: number, ofereceu?: string): number {
    this.falas.push({ quando: agora, texto, ofereceu, reacao: ofereceu ? null : "so_falou" });
    if (this.falas.length > MAX_FALAS) this.falas.shift();
    return agora; // o horário serve de id (duas falas no mesmo ms não acontecem)
  }

  /**
   * O modelo às vezes ignora "não ofereça de novo o que foi recusado" (vimos
   * ele sugerir o Madeon logo depois de um "Agora não" para o Madeon).
   * Então o CÓDIGO confere: se a nova busca tem um termo em comum com algo que
   * você recusou ou ignorou nas últimas 2h, ela é descartada.
   */
  jaRecusou(consulta: string, agora: number): boolean {
    const nova = termos(consulta);
    return this.falas.some(
      (f) =>
        f.ofereceu &&
        (f.reacao === "recusou" || f.reacao === "ignorou") &&
        agora - f.quando < 2 * 60 * 60_000 &&
        [...termos(f.ofereceu)].some((t) => nova.has(t)),
    );
  }

  registrarReacao(id: number, reacao: Reacao): void {
    const f = this.falas.find((x) => x.quando === id);
    if (f) f.reacao = reacao;
  }

  /** Minutos seguidos no programa atual. Usado também pelos "marcos de tempo". */
  minutosNoApp(agora: number): { processo: string; min: number; inicio: number } | null {
    const ultimo = this.trechos.at(-1);
    if (!ultimo || agora - ultimo.fim > BURACO_MS) return null;

    // Anda para trás enquanto for o mesmo programa e não houver buraco grande.
    let inicio = ultimo.inicio;
    for (let i = this.trechos.length - 2; i >= 0; i--) {
      const t = this.trechos[i];
      const seguinte = this.trechos[i + 1];
      if (t.processo !== ultimo.processo || seguinte.inicio - t.fim > BURACO_MS) break;
      inicio = t.inicio;
    }
    return { processo: ultimo.processo, min: Math.floor((agora - inicio) / 60_000), inicio };
  }

  resumo(c: Contexto, agora: number): ResumoMemoria {
    const data = new Date(agora);
    const r: ResumoMemoria = {
      agora: {
        hora: data.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" }),
        dia: data.toLocaleDateString("pt-BR", { weekday: "long" }),
        // Modelos pequenos erram ao deduzir "é tarde?" a partir de "00:47".
        // Entregamos a conclusão pronta.
        periodo: periodoDoDia(data.getHours()),
      },
      ultimos_30_min: this.appsRecentes(agora, 30 * 60_000),
      faixas_recentes: this.faixas
        .slice(-5)
        .reverse()
        .map((f) => (f.artista ? `${f.artista} — ${f.titulo}` : f.titulo)),
      suas_falas_recentes: this.falas
        .filter((f) => agora - f.quando < 2 * 60 * 60_000)
        .map((f) => ({
          ha_min: Math.round((agora - f.quando) / 60_000),
          fala: f.texto,
          ofereceu: f.ofereceu,
          reacao: f.reacao,
        })),
    };

    const ultimo = this.trechos.at(-1);
    if (c.janela && ultimo && ultimo.titulo === c.janela.titulo) {
      r.min_nesta_janela = Math.floor((agora - ultimo.inicio) / 60_000);
      r.min_neste_app = this.minutosNoApp(agora)?.min;
    }

    if (c.midia?.artista) {
      let n = 0;
      for (let i = this.faixas.length - 1; i >= 0 && this.faixas[i].artista === c.midia.artista; i--) n++;
      // Só manda quando É sequência. Com "1", o modelo dizia "seguido!" mesmo assim.
      if (n >= 2) r.faixas_seguidas_do_artista = n;
    }
    return r;
  }

  private appsRecentes(agora: number, janelaMs: number): { app: string; min: number }[] {
    const desde = agora - janelaMs;
    const porApp = new Map<string, number>();
    for (const t of this.trechos) {
      const ms = Math.min(t.fim, agora) - Math.max(t.inicio, desde);
      if (ms > 0) porApp.set(t.processo, (porApp.get(t.processo) ?? 0) + ms);
    }
    return [...porApp.entries()]
      .map(([app, ms]) => ({ app, min: Math.round(ms / 60_000) }))
      .filter((a) => a.min >= 1)
      .sort((a, b) => b.min - a.min)
      .slice(0, 4);
  }
}

function periodoDoDia(h: number): string {
  if (h < 5) return "madrugada";
  if (h < 12) return "manhã";
  if (h < 18) return "tarde";
  return "noite";
}
