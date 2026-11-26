/**
 * Ponto de entrada do front.
 * Fase 1: personagem, balão e click-through.
 * Fase 2: lê o contexto (janela ativa + mídia).
 * Fase 3: modelo + validação + fluxo balão → permissão ("Sim") → ação.
 * Fase 4: regras de frequência, "Agora não", não perturbe (bandeja) e config.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  aoMudarConfig,
  aoMudarNaoPerturbe,
  carregarConfig,
  definirNaoPerturbe,
  lerNaoPerturbe,
  type Config,
} from "./config";
import { obterContexto, observarContexto, type Contexto } from "./contexto";
import { executarAcao, ollamaNoAr, pensar } from "./ia/cerebro";
import type { RespostaModelo } from "./ia/esquema";
import type { Mudanca } from "./ia/prompt";
import { iniciarClickThrough } from "./janela/clickThrough";
import { posicionarNoCanto } from "./janela/posicao";
import { Memoria, termos } from "./memoria";
import { assuntoDe, Regras } from "./regras";
import { Balao } from "./ui/balao";
import { Personagem } from "./ui/personagem";

const personagem = new Personagem(document.querySelector<HTMLElement>("#buddy")!);
const balao = new Balao(document.querySelector<HTMLElement>("#balao")!);

/** Config atual. `let` porque é trocada quando você recarrega pela bandeja. */
let config: Config;

const regras = new Regras(() => ({
  intervaloEntreFalasMs: config.intervalo_entre_falas_seg * 1000,
  silencioAposRecusaMs: config.silencio_apos_recusa_min * 60_000,
}));

/** Fase 6: o que você fez nos últimos minutos e o que ele já disse. */
const memoria = new Memoria();

/**
 * Marcos de tempo: ao passar destes minutos seguidos no mesmo programa, o
 * Buddy tem um motivo para puxar assunto mesmo sem nada mudar na tela.
 * (Ainda passa pelas regras: não perturbe, intervalo, "Agora não".)
 */
const MARCOS_MIN = [45, 90, 150, 240];
const marcosDisparados = new Set<string>();

/** Último contexto recebido (já sem a janela do próprio Buddy). */
let contextoAtual: Contexto | null = null;
/** true enquanto espera o modelo: nunca fazemos duas consultas ao mesmo tempo. */
let pensando = false;
let timerEstavel: number | undefined;

async function iniciar(): Promise<void> {
  config = await carregarConfig();
  console.log("[buddy] config:", config);
  await aoMudarConfig((nova) => {
    config = nova;
    console.log("[buddy] config recarregada:", nova);
  });

  // "Não perturbe" mora no Rust (é o menu da bandeja). Lemos o estado atual e
  // escutamos as mudanças.
  aplicarNaoPerturbe(await lerNaoPerturbe());
  await aoMudarNaoPerturbe(aplicarNaoPerturbe);

  // "Esquecer tudo agora", vindo da janela de Configurações.
  await listen("esquecer-memoria", () => {
    memoria.limpar();
    marcosDisparados.clear();
    console.log("[buddy] memória apagada a pedido");
  });

  await posicionarNoCanto();

  await iniciarClickThrough({
    // O balão só é "sólido" enquanto aparece. Quando some, o clique passa por ali.
    solidos: () => (balao.visivel ? [personagem.el, balao.el] : [personagem.el]),
    aoMoverCursor: (x, y) => personagem.olharPara(x, y),
  });

  configurarCliqueEArraste();

  observarContexto({
    intervaloMs: () => config.leitura_contexto_seg * 1000,
    aoLer: (c) => memoria.registrarContexto(c, Date.now()),
    aoMudar: (novo, anterior) => {
      contextoAtual = novo;
      console.log("[buddy] contexto mudou:", JSON.stringify(novo, null, 2));
      // A primeira leitura (ao abrir o app) não é "mudança".
      if (!anterior) return;

      // Janela privada (WhatsApp, banco, e-mail...): cancela qualquer fala
      // agendada e fica quieto. Nem a música vira assunto enquanto isso.
      if (novo.privado) {
        window.clearTimeout(timerEstavel);
        return;
      }

      const foco = mesmaMidia(novo, anterior) ? "janela" : "midia";
      // Debounce: cada mudança reinicia o relógio; só pensamos quando parar de mudar.
      window.clearTimeout(timerEstavel);
      timerEstavel = window.setTimeout(
        () => void conversarAutomatico(foco),
        config.estabilizar_seg * 1000,
      );
    },
  });

  // Uma vez por minuto, confere se você cruzou um marco de tempo.
  window.setInterval(verificarMarcos, 60_000);

  // LED de status: confere se o Ollama está no ar agora e a cada minuto.
  // (Custa quase nada: /api/version não carrega modelo.)
  const checarOllama = async () => personagem.definirOffline(!(await ollamaNoAr()));
  void checarOllama();
  window.setInterval(() => void checarOllama(), 60_000);

  if (import.meta.env.DEV) {
    // Depuração: toda linha "[buddy] ..." do console também vai para o terminal
    // do `npm run tauri dev` (comando log_front no Rust). Assim dá para ver o
    // que aconteceu sem abrir o DevTools.
    for (const nivel of ["log", "warn"] as const) {
      const original = console[nivel].bind(console);
      console[nivel] = (...args: unknown[]) => {
        original(...args);
        if (typeof args[0] === "string" && args[0].startsWith("[buddy]")) {
          const linha = args.map((a) => (typeof a === "string" ? a : JSON.stringify(a))).join(" ");
          void invoke("log_front", { linha }).catch(() => {});
        }
      };
    }

    // Para depurar no console:  await buddyContexto()  ou  buddyMemoria()
    const w = window as unknown as Record<string, unknown>;
    w.buddyContexto = obterContexto;
    w.buddyMemoria = () => contextoAtual && memoria.resumo(contextoAtual, Date.now());
  }
}

function verificarMarcos(): void {
  const c = contextoAtual;
  const app = memoria.minutosNoApp(Date.now());
  if (!c || c.privado || !app) return;

  // A chave inclui o INÍCIO da sessão: se você sair e voltar ao app, os marcos recomeçam.
  const sessao = `${app.processo}@${app.inicio}`;
  const cruzados = MARCOS_MIN.filter((m) => app.min >= m && !marcosDisparados.has(`${sessao}:${m}`));
  if (cruzados.length === 0) return;

  // Marca todos os cruzados (se o PC acordou depois de 3h, não dispara 4 vezes).
  for (const m of cruzados) marcosDisparados.add(`${sessao}:${m}`);
  console.log(`[buddy] marco de tempo: ${app.min} min em ${app.processo}`);
  void conversarAutomatico("tempo");
}

function aplicarNaoPerturbe(ligado: boolean): void {
  const mudou = regras.naoPerturbe !== ligado;
  regras.naoPerturbe = ligado;
  personagem.dormir(ligado);
  if (ligado) {
    window.clearTimeout(timerEstavel);
    balao.esconder();
  } else if (mudou) {
    personagem.humor("feliz", 1000); // acordou: pulinho
  }
  console.log(`[buddy] não perturbe: ${ligado ? "ligado" : "desligado"}`);
}

function mesmaMidia(a: Contexto, b: Contexto): boolean {
  const chave = (c: Contexto) => (c.midia ? `${c.midia.artista}|${c.midia.titulo}` : "");
  return chave(a) === chave(b);
}

/** Fala automática: passa pelas regras ANTES de gastar uma chamada ao modelo. */
async function conversarAutomatico(foco: "janela" | "midia" | "tempo"): Promise<void> {
  const c = contextoAtual;
  if (!c || c.privado || pensando || balao.visivel) return;

  const assunto = assuntoDe(c, foco);
  const motivo = regras.motivoParaNaoFalar(assunto, Date.now());
  if (motivo) {
    console.log(`[buddy] não vou falar sobre ${assunto}: ${motivo}`);
    return;
  }
  await conversar(c, foco, assunto, false);
}

/** Clique no Buddy: ignora o intervalo mínimo (você pediu), mas não o "não perturbe". */
async function conversarPorClique(): Promise<void> {
  if (regras.naoPerturbe) {
    await dizer("Tô dormindo 😴 Dois cliques em mim (ou o ícone perto do relógio) me acordam.");
    return;
  }
  const c = contextoAtual;
  if (!c || pensando) return;
  const foco = c.midia?.tocando ? "midia" : "janela";
  await conversar(c, "clique", assuntoDe(c, foco), true);
}

async function conversar(c: Contexto, mudanca: Mudanca, assunto: string, manual: boolean): Promise<void> {
  pensando = true;
  regras.registrarConsulta(Date.now());
  personagem.humor("pensando");

  try {
    const tentar = async () => {
      const bruta = await pensar(c, mudanca, memoria.resumo(c, Date.now()));
      return bruta && filtrar(bruta, mudanca);
    };
    let resposta = await tentar();
    personagem.definirOffline(false); // respondeu: está no ar
    // No clique você está esperando uma resposta: se foi descartada pelos
    // filtros, tentamos de novo (~2s cada) antes do "me deu um branco".
    // Medido no caso real: ~1 em 4 respostas cai num filtro; com 3 tentativas,
    // o "branco" fica abaixo de 2%. (Na fala automática não: ficar quieto é ótimo.)
    for (let i = 0; manual && !resposta?.falar && i < 2; i++) resposta = await tentar();
    if (!resposta || !resposta.falar) {
      personagem.humor("normal");
      if (manual) await dizer("Hmm… me deu um branco agora. Tenta de novo daqui a pouco?");
      return;
    }
    // Entre a pergunta e a resposta (alguns segundos) você pode ter ligado o
    // "não perturbe". Nesse caso, a resposta é descartada.
    if (regras.naoPerturbe) {
      personagem.humor("normal");
      return;
    }
    regras.registrarFala(Date.now());
    // Não esperamos o balão fechar para liberar `pensando`.
    void falar(resposta, assunto);
  } catch (erro) {
    console.warn("[buddy] falha ao consultar o modelo:", erro);
    // Só marca offline se o Ollama realmente não responde (o erro pode ter sido
    // outro, como modelo não baixado).
    personagem.definirOffline(!(await ollamaNoAr()));
    personagem.humor("normal");
    // Só incomoda o usuário com erro se ELE pediu (clique).
    if (manual) await dizer("Não consegui falar com o Ollama. Ele está aberto?");
  } finally {
    pensando = false;
  }
}

/**
 * Correções que o CÓDIGO faz na resposta do modelo (já validada pelo zod),
 * para coisas que vimos o modelo errar mesmo com o prompt pedindo o contrário.
 */
function filtrar(r: RespostaModelo, mudanca: Mudanca): RespostaModelo | null {
  if (!r.falar) return r;

  // O qwen (modelo chinês) às vezes "escorrega" para o chinês no meio da frase.
  // Qualquer caractere de escrita asiática → descarta.
  if (/[぀-ヿ㐀-鿿가-힯]/.test(r.fala)) {
    console.log(`[buddy] descartei fala em outro idioma: "${r.fala}"`);
    return null;
  }

  if (!r.acao) {
    // Bug visto na prática: "Quer ouvir algo do mesmo estilo?" SEM ação, então o
    // balão mostrava uma pergunta com só um botão "Ok". Oferta sem ação = descarta.
    if (/\bquer (ouvir|escutar|conhecer|ver|explorar|descobrir|testar|que eu|saber|assistir|curtir|conferir|dar uma)\b/i.test(r.fala)) {
      console.log(`[buddy] descartei oferta sem ação: "${r.fala}"`);
      return null;
    }
    return r;
  }

  // Marco de tempo é para comentar/sugerir pausa. O modelo inventava buscas
  // como "pausa para café", então nesse caso a fala fica, mas sem ação.
  if (mudanca === "tempo") return { ...r, acao: null };

  // Sugestão do MESMO artista que está tocando: o prompt pede um diferente, mas
  // o modelo nem sempre percebe (bug real: no YouTube o "artista" do Windows é o
  // CANAL, e o artista de verdade estava no título: "MADEON - electric cleaners mix").
  if (mesmoArtistaTocando(r.acao.consulta)) {
    console.log(`[buddy] descartei sugestão do mesmo artista: "${r.acao.consulta}"`);
    return null;
  }

  // Sugestão de algo que você recusou/ignorou há pouco: descarta tudo.
  // (Só tirar a ação deixaria uma pergunta "Quer ouvir X?" sem botões.)
  if (memoria.jaRecusou(r.acao.consulta, Date.now())) {
    console.log(`[buddy] descartei sugestão repetida: "${r.acao.consulta}"`);
    return null;
  }
  return r;
}

/**
 * A sugestão é do artista que já está tocando? Olha o campo "artista" E o
 * começo do título (antes de " - " ou " | "), que é onde o YouTube costuma
 * colocar o artista. Compara só nomes (ignora "mix", "live", "set"...).
 */
function mesmoArtistaTocando(consulta: string): boolean {
  const m = contextoAtual?.midia;
  if (!m) return false;
  const nomes = [m.artista, m.titulo.split(/s[-–—|]s/)[0]];
  const nova = termos(consulta);
  return nomes.some((n) => [...termos(n)].some((t) => nova.has(t)));
}

function dizer(texto: string) {
  return balao.dizer(texto, { duracaoMs: Math.min(config.balao_seg, 8) * 1000 });
}

/** Contador de conversas: evita que o fim de uma conversa antiga bagunce a nova. */
let conversaAtual = 0;

/**
 * O fluxo de permissão. A ação SÓ roda se a resposta for exatamente "sim",
 * que só existe quando você clica no botão. A ação executada é a MESMA que
 * foi mostrada no balão (fica guardada nesta função, não é pedida de novo ao modelo).
 */
async function falar(r: RespostaModelo, assunto: string): Promise<void> {
  const id = ++conversaAtual;
  personagem.humor("falando");
  const duracaoMs = config.balao_seg * 1000;
  const idFala = memoria.registrarFala(r.fala, Date.now(), r.acao?.consulta);

  if (!r.acao) {
    await balao.dizer(r.fala, { duracaoMs });
    if (id === conversaAtual) personagem.humor("normal");
    return;
  }

  const acao = r.acao;
  // Se tem música tocando, o "Sim" também pausa (como no vídeo que inspirou o Buddy).
  // O balão AVISA antes: seu "Sim" vale para exatamente o que está escrito.
  const pausar = contextoAtual?.midia?.tocando === true;
  // O detalhe diz EXATAMENTE o que o "Sim" vai fazer (depende da config).
  const oQue =
    config.destino_sugestoes === "youtube_tocar"
      ? `▶ toco "${acao.consulta}" no YouTube`
      : config.destino_sugestoes === "youtube_busca"
        ? `🔎 busco "${acao.consulta}" no YouTube`
        : `🔎 busco "${acao.consulta}"`;
  const detalhe = pausar ? `⏸ pauso a música · ${oQue}` : oQue;
  const resposta = await balao.perguntar(r.fala, { detalhe, duracaoMs });
  console.log("[buddy] resposta do usuário:", resposta, acao);
  if (id !== conversaAtual) return; // foi substituída por outra conversa

  // A memória lembra sua reação: o modelo vê isso na próxima vez, e o filtro
  // `jaRecusou` impede repetir o que você recusou ou deixou passar.
  memoria.registrarReacao(idFala, resposta === "sim" ? "aceitou" : resposta === "nao" ? "recusou" : "ignorou");

  if (resposta === "sim") {
    try {
      await executarAcao(acao, pausar);
      personagem.humor("feliz", 1200);
    } catch (erro) {
      console.warn("[buddy] ação falhou:", erro);
      personagem.humor("normal");
      await dizer("Ops, não consegui abrir o navegador.");
    }
  } else if (resposta === "nao") {
    // Respeita o "Agora não": este assunto fica bloqueado por um bom tempo.
    regras.registrarRecusa(assunto, Date.now());
    console.log(`[buddy] ok, sem falar de ${assunto} por ${config.silencio_apos_recusa_min} min`);
    personagem.humor("triste", 2500);
  } else {
    personagem.humor("normal");
  }
}

/**
 * Clique e arraste no mesmo elemento: se o mouse andar mais de 4px com o botão
 * apertado, é arraste (move a janela); se soltar antes disso, é clique.
 */
function configurarCliqueEArraste(): void {
  const LIMIAR = 4;
  let inicio: { x: number; y: number } | null = null;

  personagem.el.addEventListener("mousedown", (e) => {
    if (e.button === 0) inicio = { x: e.screenX, y: e.screenY };
  });

  window.addEventListener("mousemove", (e) => {
    if (!inicio) return;
    if (Math.hypot(e.screenX - inicio.x, e.screenY - inicio.y) > LIMIAR) {
      inicio = null;
      // Entrega o arraste para o Windows. Depois disso a página não recebe o
      // mouseup (o sistema "engole"), por isso zeramos `inicio` antes.
      void getCurrentWindow().startDragging();
    }
  });

  // Clique simples x duplo: o clique simples espera um pouquinho para ver se
  // vem um segundo. Sem essa espera, todo duplo clique também dispararia uma
  // conversa (e uma chamada ao modelo) logo antes de dormir.
  const JANELA_DUPLO_MS = 280;
  let timerClique: number | undefined;

  window.addEventListener("mouseup", () => {
    if (!inicio) return;
    inicio = null;

    if (timerClique !== undefined) {
      // Segundo clique dentro do prazo: é duplo clique → dorme/acorda.
      window.clearTimeout(timerClique);
      timerClique = undefined;
      void definirNaoPerturbe(!regras.naoPerturbe);
      return;
    }
    timerClique = window.setTimeout(() => {
      timerClique = undefined;
      void conversarPorClique();
    }, JANELA_DUPLO_MS);
  });
}

void iniciar();
