/**
 * O "contrato" da resposta do modelo, escrito com zod.
 *
 * zod descreve o formato esperado e valida dados em tempo de execução.
 * O TypeScript sozinho NÃO faz isso: os tipos somem quando o código vira JS,
 * então `JSON.parse(texto) as RespostaModelo` seria só uma promessa sem garantia.
 * Saída de modelo de IA é o exemplo perfeito de dado em que não se confia.
 *
 * Este mesmo esquema é convertido em JSON Schema e mandado ao Ollama, que o usa
 * para forçar o formato na geração. Uma fonte de verdade, dois usos.
 */

import { z } from "zod";

/** A única ação que existe. Qualquer outro "tipo" falha na validação. */
export const AcaoAbrirBusca = z.object({
  tipo: z.literal("abrir_busca"),
  consulta: z.string().trim().min(2).max(80),
});

export const RespostaModelo = z
  .object({
    falar: z.boolean(),
    fala: z.string().trim().max(200),
    acao: AcaoAbrirBusca.nullable(),
  })
  // Regras que envolvem mais de um campo:
  .refine((r) => !r.falar || r.fala.length > 0, {
    message: "falar=true com fala vazia",
  })
  .refine((r) => r.acao === null || r.falar, {
    message: "ação sem fala (o usuário precisa ser perguntado)",
  });

export type RespostaModelo = z.infer<typeof RespostaModelo>;
export type Acao = z.infer<typeof AcaoAbrirBusca>;

/** JSON Schema para o parâmetro `format` do Ollama. */
export const formatoOllama = z.toJSONSchema(RespostaModelo);

/**
 * Converte o texto cru do modelo em uma resposta válida, ou `null`.
 * Regra do projeto: resposta inválida é ignorada EM SILÊNCIO (só log no console).
 */
export function interpretarResposta(texto: string): RespostaModelo | null {
  let json: unknown;
  try {
    json = JSON.parse(texto);
  } catch {
    console.warn("[buddy] modelo devolveu algo que não é JSON:", texto);
    return null;
  }

  const resultado = RespostaModelo.safeParse(json);
  if (!resultado.success) {
    console.warn("[buddy] JSON do modelo fora do formato:", z.prettifyError(resultado.error), json);
    return null;
  }
  return resultado.data;
}
