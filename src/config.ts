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

/** Chamado quando você usa "Recarregar configuração" na bandeja. */
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
