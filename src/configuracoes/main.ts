/**
 * Janela de Configurações.
 *
 * Fluxo: cada mudança é salva NA HORA (sem botão "Salvar"). O Rust valida,
 * grava o config.toml e avisa todas as janelas com o evento "config-alterada";
 * o Buddy aplica sem reiniciar.
 *
 * Esta janela tem permissões mínimas (capabilities/config.json): ela não
 * consegue mexer em janelas, nem fazer requisições de rede, nem nada além de
 * chamar os comandos do app, que validam tudo do lado do Rust.
 */

import { emit } from "@tauri-apps/api/event";
import {
  abrirArquivoConfig,
  aoMudarConfig,
  aoMudarNaoPerturbe,
  carregarConfig,
  definirNaoPerturbe,
  lerNaoPerturbe,
  listarModelos,
  listasPrivacidade,
  recarregarConfig,
  salvarConfig,
  type Config,
} from "../config";
import { ollamaNoAr } from "../ollama";

const $ = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector<T>(sel)!;

let config: Config;

// ---------------------------------------------------------------- navegação

function irPara(tela: string): void {
  document.querySelectorAll<HTMLElement>(".tela").forEach((t) => {
    t.hidden = t.id !== `tela-${tela}`;
  });
  if (tela === "ia") void carregarModelos();
}

document.querySelectorAll<HTMLElement>("[data-ir]").forEach((b) =>
  b.addEventListener("click", () => irPara(b.dataset.ir!)),
);
document.querySelectorAll(".voltar").forEach((b) => b.addEventListener("click", () => irPara("inicio")));
// Esc volta para o início (atalho comum em telas de configuração).
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") irPara("inicio");
});

// ---------------------------------------------------------------- salvar

let timerToast: number | undefined;
function avisar(texto: string): void {
  const t = $("#toast");
  t.textContent = texto;
  t.classList.add("visivel");
  window.clearTimeout(timerToast);
  timerToast = window.setTimeout(() => t.classList.remove("visivel"), 1400);
}

/** Salva só o que mudou (o resto da config vai igual). */
async function salvar(mudanca: Partial<Config>): Promise<void> {
  try {
    config = await salvarConfig({ ...config, ...mudanca });
    desenhar();
    avisar("Salvo");
  } catch (erro) {
    console.warn("[buddy] erro ao salvar:", erro);
    avisar("Não consegui salvar 😕");
  }
}

// ---------------------------------------------------------------- desenhar

const NOMES_DESTINO: Record<string, string> = {
  youtube_tocar: "Tocar no YouTube",
  youtube_busca: "Buscar no YouTube",
  web: "Buscar na web",
};

function formatarIntervalo(seg: number): string {
  if (seg < 60) return `${seg} s`;
  if (seg < 3600) return `a cada ${Math.round(seg / 60)} min`;
  return `a cada ${Math.round(seg / 3600)} h`;
}

/** Atualiza a tela inteira a partir de `config`. */
function desenhar(): void {
  const extras = config.apps_ignorados.length + config.titulos_ignorados.length;
  $("#valor-visao").textContent = extras ? `${extras} escondido${extras > 1 ? "s" : ""} por você` : "Padrão";
  $("#valor-ia").textContent = config.modelo;
  $("#valor-frequencia").textContent = formatarIntervalo(config.intervalo_entre_falas_seg);
  $("#valor-sugestoes").textContent = NOMES_DESTINO[config.destino_sugestoes] ?? "";
  $<HTMLSelectElement>("#destino").value = config.destino_sugestoes;

  desenharLista("#lista-apps", config.apps_ignorados, (resto) => salvar({ apps_ignorados: resto }));
  desenharLista("#lista-titulos", config.titulos_ignorados, (resto) => salvar({ titulos_ignorados: resto }));

  selecionar("#intervalo", config.intervalo_entre_falas_seg);
  selecionar("#silencio", config.silencio_apos_recusa_min);
  selecionar("#balao", config.balao_seg);
}

/**
 * Coloca o valor no <select>. Se o valor veio do arquivo e não é uma das
 * opções (ex.: você pôs 240 no .toml), criamos a opção, para não mentir na tela.
 */
function selecionar(sel: string, valor: number): void {
  const s = $<HTMLSelectElement>(sel);
  if (![...s.options].some((o) => o.value === String(valor))) {
    s.add(new Option(`${valor} (do arquivo)`, String(valor)));
  }
  s.value = String(valor);
}

/** Chips com "×" para remover. textContent: o nome vem de você, mas nunca vira HTML. */
function desenharLista(sel: string, itens: string[], aoRemover: (resto: string[]) => void): void {
  const ul = $(sel);
  ul.replaceChildren();
  if (itens.length === 0) {
    const li = document.createElement("li");
    li.className = "vazio";
    li.textContent = "Nenhum.";
    ul.append(li);
    return;
  }
  for (const item of itens) {
    const li = document.createElement("li");
    li.textContent = item;
    const x = document.createElement("button");
    x.type = "button";
    x.textContent = "×";
    x.title = `Remover ${item}`;
    x.addEventListener("click", () => aoRemover(itens.filter((i) => i !== item)));
    li.append(x);
    ul.append(li);
  }
}

function chips(sel: string, itens: string[]): void {
  const ul = $(sel);
  ul.replaceChildren(
    ...itens.map((i) => {
      const li = document.createElement("li");
      li.textContent = i;
      return li;
    }),
  );
}

// ---------------------------------------------------------------- formulários

function ligarFormulario(sel: string, campo: "apps_ignorados" | "titulos_ignorados"): void {
  const form = $<HTMLFormElement>(sel);
  form.addEventListener("submit", (e) => {
    e.preventDefault(); // sem isso o form recarregaria a página
    const input = form.elements.namedItem("item") as HTMLInputElement;
    const valor = input.value.trim();
    if (!valor) return;
    input.value = "";
    void salvar({ [campo]: [...config[campo], valor] });
  });
}

ligarFormulario("#form-apps", "apps_ignorados");
ligarFormulario("#form-titulos", "titulos_ignorados");

for (const [sel, campo] of [
  ["#intervalo", "intervalo_entre_falas_seg"],
  ["#silencio", "silencio_apos_recusa_min"],
  ["#balao", "balao_seg"],
] as const) {
  $<HTMLSelectElement>(sel).addEventListener("change", (e) =>
    salvar({ [campo]: Number((e.target as HTMLSelectElement).value) }),
  );
}

$<HTMLSelectElement>("#destino").addEventListener("change", (e) =>
  salvar({ destino_sugestoes: (e.target as HTMLSelectElement).value as Config["destino_sugestoes"] }),
);

$<HTMLSelectElement>("#modelo").addEventListener("change", (e) =>
  salvar({ modelo: (e.target as HTMLSelectElement).value }),
);

// Modo silencioso = o mesmo "não perturbe" da bandeja e do duplo clique.
const silencioso = $<HTMLInputElement>("#silencioso");
silencioso.addEventListener("change", () => void definirNaoPerturbe(silencioso.checked));

$("#esquecer").addEventListener("click", async () => {
  // A memória mora na janela do Buddy; mandamos um evento para ela.
  await emit("esquecer-memoria");
  $("#aviso-memoria").hidden = false;
});

$("#abrir-arquivo").addEventListener("click", () => void abrirArquivoConfig());
$("#recarregar-arquivo").addEventListener("click", async () => {
  config = await recarregarConfig();
  desenhar();
  avisar("Recarregado do arquivo");
});

// ---------------------------------------------------------------- IA / status

async function carregarModelos(): Promise<void> {
  const select = $<HTMLSelectElement>("#modelo");
  const dica = $("#dica-ia");
  try {
    const modelos = await listarModelos();
    // Se o modelo da config não estiver baixado, mostramos mesmo assim (com aviso).
    const todos = modelos.includes(config.modelo) ? modelos : [config.modelo, ...modelos];
    select.replaceChildren(
      ...todos.map((m) => new Option(modelos.includes(m) ? m : `${m} (não baixado)`, m)),
    );
    select.value = config.modelo;
    select.disabled = false;
    dica.textContent = "Modelos maiores são mais espertos e mais lentos. Para baixar outro: ollama pull <nome>";
  } catch {
    select.replaceChildren(new Option(config.modelo, config.modelo));
    select.disabled = true;
    dica.textContent = "Não consegui falar com o Ollama. Ele está aberto?";
  }
}

async function atualizarEstado(): Promise<void> {
  const ok = await ollamaNoAr();
  $("#bolinha").className = `bolinha ${ok ? "ok" : "erro"}`;
  $("#estado-texto").textContent = ok ? "Online · IA local" : "Offline · o Ollama não responde";
  $("#mini-led").setAttribute("fill", ok ? "#FF7849" : "#EF4444");
}

// ---------------------------------------------------------------- início

async function iniciar(): Promise<void> {
  config = await carregarConfig();
  desenhar();

  silencioso.checked = await lerNaoPerturbe();
  await aoMudarNaoPerturbe((ligado) => (silencioso.checked = ligado));
  // Se a config mudar por outro caminho (arquivo recarregado), a tela acompanha.
  await aoMudarConfig((nova) => {
    config = nova;
    desenhar();
  });

  const fixos = await listasPrivacidade();
  chips("#fixos-apps", fixos.apps);
  chips("#fixos-titulos", fixos.titulos);
  $("#resumo-fixos").textContent =
    `Sempre escondidos (${fixos.apps.length} programas, ${fixos.titulos.length} sites/títulos)`;

  await atualizarEstado();
  // Enquanto a janela está aberta, confere o Ollama de tempos em tempos.
  window.setInterval(() => void atualizarEstado(), 10_000);
}

void iniciar();
