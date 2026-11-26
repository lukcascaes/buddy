/**
 * Ponte entre o TypeScript e o comando Rust `get_context` (src-tauri/src/contexto.rs).
 *
 * Os tipos abaixo ESPELHAM as structs do Rust. Se mudar lá, mude aqui.
 * (Dá para gerar automaticamente com crates como `ts-rs`/`specta`, mas para um
 * projeto deste tamanho escrever à mão é mais simples e você aprende mais.)
 */

import { invoke } from "@tauri-apps/api/core";

export interface JanelaAtiva {
  titulo: string;
  /** Nome do executável, ex.: "chrome.exe" */
  processo: string;
}

export interface Midia {
  titulo: string;
  artista: string;
  /** App de origem, ex.: "Spotify.exe" */
  app: string;
  tocando: boolean;
}

export interface Contexto {
  /** null quando a janela ativa é o próprio Buddy, não tem título ou é privada. */
  janela: JanelaAtiva | null;
  midia: Midia | null;
  /**
   * true quando você está numa janela PRIVADA (WhatsApp, banco, e-mail...).
   * O Rust já apagou o título; aqui só sabemos "é privada", nada mais.
   * Nesse estado o Buddy não puxa assunto (nem sobre a música).
   */
  privado: boolean;
}

export function obterContexto(): Promise<Contexto> {
  return invoke<Contexto>("get_context");
}

/**
 * "Assinatura" do contexto: uma string que só muda quando algo RELEVANTE muda.
 * Na Fase 3/4, o Buddy só vai consultar o modelo quando esta assinatura mudar.
 *
 * Note o que fica DE FORA:
 * - `tocando` (pausar e despausar a mesma música não é assunto novo);
 * - o processo da janela (o título já diferencia).
 */
export function assinatura(c: Contexto): string {
  const j = c.janela ? c.janela.titulo : "";
  const m = c.midia ? `${c.midia.artista} — ${c.midia.titulo}` : "";
  return `${c.privado ? "privado" : j}|${m}`;
}

export interface OpcoesObservador {
  /** Intervalo entre leituras (ms). Função para refletir a config recarregada. */
  intervaloMs: () => number;
  /** Chamado só quando a assinatura muda. */
  aoMudar: (novo: Contexto, anterior: Contexto | null) => void;
  /** Chamado a CADA leitura (a memória precisa saber "continua igual"). */
  aoLer?: (c: Contexto) => void;
}

/**
 * Lê o contexto periodicamente e avisa quando ele muda.
 *
 * Detalhe: quando você clica no Buddy, a janela ativa vira ele mesmo e o Rust
 * devolve `janela: null`. Nesse caso mantemos a ÚLTIMA janela conhecida, senão
 * cada clique no personagem pareceria "mudança de contexto".
 */
export function observarContexto(op: OpcoesObservador): void {
  let atual: Contexto | null = null;
  let ultimaJanela: JanelaAtiva | null = null;

  const ler = async (): Promise<void> => {
    try {
      const bruto = await obterContexto();
      // Ao entrar numa janela privada, esquecemos a anterior também: senão,
      // clicando no Buddy de dentro do WhatsApp, ele comentaria a janela de antes
      // como se você ainda estivesse nela.
      if (bruto.privado) ultimaJanela = null;
      else if (bruto.janela) ultimaJanela = bruto.janela;
      const novo: Contexto = {
        janela: bruto.privado ? null : (bruto.janela ?? ultimaJanela),
        midia: bruto.midia,
        privado: bruto.privado,
      };
      op.aoLer?.(novo);

      if (atual === null || assinatura(novo) !== assinatura(atual)) {
        const anterior = atual;
        atual = novo;
        op.aoMudar(novo, anterior);
      } else {
        atual = novo; // atualiza detalhes como `tocando` sem disparar evento
      }
    } catch (erro) {
      console.warn("[buddy] erro lendo contexto:", erro);
    }
    window.setTimeout(ler, op.intervaloMs());
  };

  void ler();
}
