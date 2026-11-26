/**
 * O personagem: controla o "humor" (classes CSS) e faz os olhos seguirem o mouse.
 * Toda a animação em si está no styles.css; aqui só ligamos/desligamos classes.
 */

export type Humor = "normal" | "pensando" | "falando" | "feliz" | "triste";

/** Quanto a pupila pode se deslocar do centro do olho, em unidades do SVG. */
const ALCANCE_PUPILA = 3;

export class Personagem {
  readonly el: HTMLElement;
  private pupilas: SVGElement[];
  private timerHumor: number | undefined;
  private ultimoOlhar = "";

  constructor(el: HTMLElement) {
    this.el = el;
    this.pupilas = Array.from(el.querySelectorAll<SVGElement>(".pupila"));
    this.agendarPiscada();
    this.agendarMexida();
  }

  /**
   * Em repouso ele fica parado (animação contínua custa GPU, ver styles.css).
   * De vez em quando (15 a 35 s) dá UMA flutuada, de 3,2 s, para parecer vivo.
   * Média: anima ~13% do tempo em vez de 100%.
   */
  private agendarMexida(): void {
    const espera = 15_000 + Math.random() * 20_000;
    window.setTimeout(() => {
      this.el.classList.add("mexendo");
      window.setTimeout(() => this.el.classList.remove("mexendo"), 3_200);
      this.agendarMexida();
    }, espera);
  }

  /**
   * Pisca em intervalos aleatórios (2,5 a 6 s), como gente de verdade.
   * Por que JS e não uma animação CSS infinita? A animação CSS "roda" o tempo todo,
   * mesmo nos ~95% do ciclo em que o olho está parado, e dentro de um SVG isso
   * custa redesenho a cada quadro. Aqui só gastamos algo nos 150ms da piscada.
   */
  private agendarPiscada(): void {
    const espera = 2500 + Math.random() * 3500;
    window.setTimeout(() => {
      this.el.classList.add("piscando");
      window.setTimeout(() => this.el.classList.remove("piscando"), 150);
      this.agendarPiscada();
    }, espera);
  }

  /**
   * Troca o humor. Se `duracaoMs` for passado, volta para "normal" depois.
   * Ex.: humor("feliz", 1200) dá um pulinho e volta ao normal.
   */
  humor(novo: Humor, duracaoMs?: number): void {
    this.el.classList.remove("pensando", "falando", "feliz", "triste");
    if (novo !== "normal") this.el.classList.add(novo);

    window.clearTimeout(this.timerHumor);
    if (duracaoMs !== undefined) {
      this.timerHumor = window.setTimeout(() => this.humor("normal"), duracaoMs);
    }
  }

  /** Modo "não perturbe": olhos fechados, meio transparente, flutuando devagar. */
  dormir(ligado: boolean): void {
    this.el.classList.toggle("dormindo", ligado);
  }

  /** LED vermelho quando o Ollama não responde. A dica do mouse explica o porquê. */
  definirOffline(offline: boolean): void {
    if (this.el.classList.contains("offline") === offline) return;
    this.el.classList.toggle("offline", offline);
    this.el.title = offline
      ? "Não consigo falar com o Ollama. Ele está aberto?"
      : "Clique para falar comigo · dois cliques para eu dormir · arraste para mover";
  }

  /**
   * Faz as pupilas olharem para um ponto (x, y) em pixels CSS relativos à janela.
   * O ponto pode estar FORA da janela (o mouse em qualquer lugar da tela):
   * é isso que dá a sensação de que ele está "olhando" você trabalhar.
   */
  olharPara(x: number, y: number): void {
    const r = this.el.getBoundingClientRect();
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height * 0.5;
    const dx = x - cx;
    const dy = y - cy;
    const dist = Math.hypot(dx, dy);

    // Perto do personagem a pupila se move pouco; longe, chega ao limite.
    // (dist / 150) cresce com a distância e o Math.min trava em 1.
    const forca = Math.min(dist / 150, 1) * ALCANCE_PUPILA;
    const px = dist === 0 ? 0 : (dx / dist) * forca;
    const py = dist === 0 ? 0 : (dy / dist) * forca;

    // Só mexe no DOM se a posição mudou: com o mouse parado, nada é redesenhado.
    // toFixed(1): uma casa decimal basta; evita mexer no DOM por diferenças invisíveis.
    const t = `translate(${px.toFixed(1)}px, ${py.toFixed(1)}px)`;
    if (t === this.ultimoOlhar) return;
    this.ultimoOlhar = t;
    for (const p of this.pupilas) p.style.transform = t;
  }
}
