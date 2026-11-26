/**
 * Balão de fala.
 *
 * A API é baseada em Promise para o fluxo ficar legível nas próximas fases:
 *
 *   const resposta = await balao.perguntar("Quer algo parecido?");
 *   if (resposta === "sim") executarAcao(...);
 *
 * Ou seja: a ação SÓ acontece depois de um clique explícito em "Sim".
 * "timeout" (sumiu sozinho) e "nao" nunca disparam ação.
 */

export type Resposta = "sim" | "nao" | "timeout";

export interface OpcoesBalao {
  /** Mostra os botões Sim / Agora não. Se false, mostra só um "Ok". */
  pergunta: boolean;
  /** Tempo até o balão sumir sozinho. Pausa enquanto o mouse está em cima. */
  duracaoMs: number;
  /** Linha pequena abaixo da fala, ex.: "🔎 artistas parecidos com X". */
  detalhe?: string;
}

const PADRAO: OpcoesBalao = { pergunta: true, duracaoMs: 12_000 };

export class Balao {
  readonly el: HTMLElement;
  private texto: HTMLElement;
  private detalhe: HTMLElement;
  private btnSim: HTMLButtonElement;
  private btnNao: HTMLButtonElement;
  private barra: HTMLElement;

  /** Função que finaliza a pergunta atual (resolve a Promise pendente). */
  private finalizar: ((r: Resposta) => void) | null = null;

  constructor(el: HTMLElement) {
    this.el = el;
    this.texto = el.querySelector("#balao-texto")!;
    this.detalhe = el.querySelector("#balao-detalhe")!;
    this.btnSim = el.querySelector("#btn-sim")!;
    this.btnNao = el.querySelector("#btn-nao")!;
    this.barra = el.querySelector("#balao-barra")!;

    this.btnSim.addEventListener("click", () => this.finalizar?.("sim"));
    this.btnNao.addEventListener("click", () => this.finalizar?.("nao"));
    // Quando a animação da barrinha termina, o tempo acabou.
    this.barra.addEventListener("animationend", () => this.finalizar?.("timeout"));
  }

  get visivel(): boolean {
    return this.el.classList.contains("visivel");
  }

  /** Mostra uma pergunta com Sim / Agora não e espera a resposta. */
  perguntar(texto: string, opcoes: Partial<OpcoesBalao> = {}): Promise<Resposta> {
    return this.mostrar(texto, { ...PADRAO, ...opcoes, pergunta: true });
  }

  /** Mostra uma fala simples, só com "Ok". */
  dizer(texto: string, opcoes: Partial<OpcoesBalao> = {}): Promise<Resposta> {
    return this.mostrar(texto, { ...PADRAO, duracaoMs: 7_000, ...opcoes, pergunta: false });
  }

  /** Esconde o balão (conta como "timeout" se havia uma pergunta pendente). */
  esconder(): void {
    this.finalizar?.("timeout");
  }

  private mostrar(texto: string, op: OpcoesBalao): Promise<Resposta> {
    // Se já havia algo na tela, encerra a pergunta anterior sem ação.
    this.finalizar?.("timeout");

    // SEGURANÇA: textContent, NUNCA innerHTML. O texto vai vir do modelo de IA,
    // que por sua vez lê títulos de janela (que qualquer site pode controlar).
    // Com innerHTML, um título tipo "<img src=x onerror=...>" viraria código rodando aqui.
    this.texto.textContent = texto;
    this.detalhe.textContent = op.detalhe ?? "";
    this.detalhe.hidden = !op.detalhe;

    this.btnSim.textContent = op.pergunta ? "Sim" : "Ok";
    this.btnNao.hidden = !op.pergunta;

    // Reinicia a animação da barra: remove a classe, força o navegador a
    // "perceber" (ler offsetWidth faz isso) e coloca de novo.
    this.el.style.setProperty("--duracao", `${op.duracaoMs}ms`);
    this.el.classList.remove("contando");
    void this.el.offsetWidth;
    this.el.classList.add("contando", "visivel");
    this.el.setAttribute("aria-hidden", "false");

    return new Promise<Resposta>((resolve) => {
      this.finalizar = (r) => {
        this.finalizar = null;
        this.el.classList.remove("visivel", "contando");
        this.el.setAttribute("aria-hidden", "true");
        // Num balão sem pergunta, "Ok" não é um "sim" a nenhuma ação.
        resolve(op.pergunta ? r : "timeout");
      };
    });
  }
}
