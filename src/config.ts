/**
 * Config lida pelo Rust (src-tauri/src/config.rs) a partir de
 * %APPDATA%\com.buddy.desktop\config.toml. O front só LÊ.
 *
 * As chaves ficam em snake_case porque é assim que o Rust/serde as manda.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Config {
  ollama_url: string;
  modelo: string;
  buscador: string;
  /** O que o "Sim" faz com a sugestão. */
  destino_sugestoes: "youtube_tocar" | "youtube_busca" | "web";
  intervalo_entre_falas_seg: number;
  silencio_apos_recusa_min: number;
  estabilizar_seg: number;
  leitura_contexto_seg: number;
  balao_seg: number;
  /** ALÉM da lista embutida no Rust (privacidade.rs), que não pode ser removida. */
  apps_ignorados: string[];
  titulos_ignorados: string[];
}

export function carregarConfig(): Promise<Config> {
  return invoke<Config>("get_config");
}

/** Chamado sempre que a config muda (salva pela janela ou recarregada do arquivo). */
export function aoMudarConfig(cb: (c: Config) => void): Promise<() => void> {
  return listen<Config>("config-alterada", (e) => cb(e.payload));
}

export function lerNaoPerturbe(): Promise<boolean> {
  return invoke<boolean>("get_nao_perturbe");
}

/**
 * Liga/desliga o "não perturbe" (usado pelo duplo clique). O Rust atualiza o
 * menu da bandeja e devolve o evento "nao-perturbe", então quem reage é sempre
 * o mesmo código, venha a mudança do menu ou do duplo clique.
 */
export function definirNaoPerturbe(ligado: boolean): Promise<void> {
  return invoke("set_nao_perturbe", { ligado });
}

export function aoMudarNaoPerturbe(cb: (ligado: boolean) => void): Promise<() => void> {
  return listen<boolean>("nao-perturbe", (e) => cb(e.payload));
}

// ---- Usado pela janela de Configurações ----

/** Salva e devolve a config COMO FICOU (o Rust pode ter corrigido valores). */
export function salvarConfig(nova: Config): Promise<Config> {
  return invoke<Config>("salvar_config", { nova });
}

export function recarregarConfig(): Promise<Config> {
  return invoke<Config>("recarregar_config");
}

export function abrirArquivoConfig(): Promise<void> {
  return invoke("abrir_arquivo_config");
}

/** Modelos baixados no Ollama. Lança erro se o Ollama estiver fora do ar. */
export function listarModelos(): Promise<string[]> {
  return invoke<string[]>("listar_modelos");
}

export interface ListasFixas {
  apps: string[];
  titulos: string[];
}

/** O que o Buddy ignora SEMPRE (embutido no Rust, só leitura). */
export function listasPrivacidade(): Promise<ListasFixas> {
  return invoke<ListasFixas>("listas_privacidade");
}
