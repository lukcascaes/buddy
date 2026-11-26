/**
 * Click-through: deixa o clique "atravessar" as partes vazias da janela.
 *
 * O PROBLEMA (e por que isto não é óbvio):
 * `setIgnoreCursorEvents(true)` faz o Windows ignorar o mouse na janela INTEIRA.
 * Nesse estado a página não recebe mais nenhum evento (nem mousemove), então ela
 * não tem como saber quando o mouse voltou para cima do personagem. Seria um
 * caminho sem volta.
 *
 * A SOLUÇÃO:
 * Em vez de depender de eventos do mouse, perguntamos periodicamente ao sistema
 * "onde está o cursor na tela?" (cursorPosition), convertemos para coordenadas
 * da janela e testamos se ele está sobre algum elemento "sólido"
 * (personagem ou balão visível). Só chamamos setIgnoreCursorEvents quando o estado
 * MUDA, para não fazer chamadas à toa.
 *
 * O caso do balão: ele entra na lista de sólidos só enquanto está visível. Por isso
 * dá para passar o mouse e clicar nos botões, e o hover pausa o timer de sumir.
 */

import { cursorPosition, getCurrentWindow, type PhysicalPosition } from "@tauri-apps/api/window";

/** Folga em px ao redor dos elementos. Evita "piscar" bem na borda. */
const FOLGA = 6;
/** Intervalo de verificação quando o mouse está perto da janela (ms). */
const INTERVALO_PERTO = 40;
/** Intervalo quando o mouse está longe: economiza CPU. */
const INTERVALO_LONGE = 150;
/** Distância (px) além da borda da janela que ainda conta como "perto". */
const DISTANCIA_PERTO = 120;

export interface OpcoesClickThrough {
  /** Elementos que devem receber clique neste momento. */
  solidos: () => HTMLElement[];
  /** Chamado a cada verificação com a posição do mouse (px CSS, relativo à janela). */
  aoMoverCursor?: (x: number, y: number) => void;
}

export async function iniciarClickThrough(op: OpcoesClickThrough): Promise<void> {
  const janela = getCurrentWindow();

  // Guardamos posição e escala em variáveis e atualizamos só quando mudam
  // (eventos onMoved/onScaleChanged), em vez de perguntar ao Rust a cada 40ms.
  let pos: PhysicalPosition = await janela.innerPosition();
  let escala = await janela.scaleFactor();
  await janela.onMoved(({ payload }) => (pos = payload));
  await janela.onScaleChanged(({ payload }) => (escala = payload.scaleFactor));

  // null = ainda não definimos nada; força a primeira chamada.
  let ignorando: boolean | null = null;

  const verificar = async (): Promise<void> => {
    let proximo = INTERVALO_PERTO;
    try {
      // cursorPosition() vem em pixels FÍSICOS da tela. O layout da página usa
      // pixels CSS (lógicos). Com escala de 150% no Windows, 1px CSS = 1,5px físico.
      const c = await cursorPosition();
      const x = (c.x - pos.x) / escala;
      const y = (c.y - pos.y) / escala;

      op.aoMoverCursor?.(x, y);

      const sobreSolido = op.solidos().some((el) => dentro(el, x, y, FOLGA));
      const deveIgnorar = !sobreSolido;
      if (deveIgnorar !== ignorando) {
        ignorando = deveIgnorar;
        await janela.setIgnoreCursorEvents(deveIgnorar);
      }

      const longe =
        x < -DISTANCIA_PERTO ||
        y < -DISTANCIA_PERTO ||
        x > window.innerWidth + DISTANCIA_PERTO ||
        y > window.innerHeight + DISTANCIA_PERTO;
      if (longe) proximo = INTERVALO_LONGE;
    } catch (erro) {
      console.warn("[buddy] click-through falhou nesta rodada:", erro);
      proximo = 500;
    }
    // setTimeout encadeado (e não setInterval): garante que uma verificação
    // só começa depois que a anterior terminou, mesmo se o IPC atrasar.
    window.setTimeout(verificar, proximo);
  };

  void verificar();
}

function dentro(el: HTMLElement, x: number, y: number, folga: number): boolean {
  const r = el.getBoundingClientRect();
  if (r.width === 0 || r.height === 0) return false;
  return x >= r.left - folga && x <= r.right + folga && y >= r.top - folga && y <= r.bottom + folga;
}
