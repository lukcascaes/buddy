# Buddy

Mascote de desktop para Windows que fica por cima das outras janelas, percebe o
que você está fazendo (janela ativa + música tocando) e puxa conversa sozinho,
usando uma IA **local** (Ollama). Ele **nunca** faz nada sem o seu clique em "Sim".

- Tauri v2 (Rust mínimo) + TypeScript/Vite, sem framework
- IA local via Ollama, sem API paga e sem chave
- Só texto por enquanto; a arquitetura está pronta para um modelo de visão
- Personagem: uma bolinha creme cujos olhos seguem o seu mouse, com um LED de status

---

## Requisitos (Windows)

| O quê | Para quê | Instalar |
|---|---|---|
| Node 20+ | front (Vite/TypeScript) | `winget install OpenJS.NodeJS.LTS` |
| Rust (MSVC) | backend do Tauri | `winget install Rustlang.Rustup` |
| VS Build Tools (C++) | linker que o Rust usa no Windows | `winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"` |
| Ollama | IA local | `winget install Ollama.Ollama` |
| WebView2 | motor da janela | já vem no Windows 11 |

> **Controle Inteligente de Aplicativos (Smart App Control):** se estiver ligado,
> o Windows bloqueia os executáveis que o Rust gera ao compilar
> (`os error 4551`). É preciso desligá-lo em *Segurança do Windows → Controle de
> aplicativos e do navegador*.

Depois, baixe pelo menos um modelo:

```powershell
ollama pull qwen2.5:7b
```

## Como rodar

```powershell
npm install
npm run tauri dev
```

A primeira compilação leva uns 5 minutos; as seguintes, segundos. Em modo dev,
editar arquivos `.ts`/`.css` atualiza na hora e editar `.rs` recompila e reabre.

**Usando:**
- **Clique** no Buddy: ele fala na hora sobre o que você está fazendo.
- **Duplo clique:** dorme (modo "não perturbe"). Outro duplo clique acorda.
- **Arrastar:** move o Buddy pela tela.
- **LED na bochecha:** laranja = tudo ok · pulsando = pensando ·
  vermelho = o Ollama não responde · apagado = dormindo.
- **Ícone na bandeja** (perto do relógio): não perturbe, abrir/recarregar
  configuração e **Sair**.

### Gerar o executável otimizado

```powershell
npm run tauri build
```

Saída: `src-tauri\target\release\buddy.exe` e um instalador em
`src-tauri\target\release\bundle\nsis\` (na primeira vez o Tauri baixa o NSIS
para montar o instalador). Para gerar só o `.exe`, sem instalador:
`npx tauri build --no-bundle`. O executável não é assinado, então o Windows
(SmartScreen) vai avisar em outros PCs.

## Como trocar o modelo

1. Baixe o modelo: `ollama pull llama3.2:3b` (veja os baixados com `ollama list`).
2. Bandeja → **Abrir configuração** → troque `modelo = "llama3.2:3b"` → salve.
3. Bandeja → **Recarregar configuração**. Não precisa reiniciar.

| Modelo | Tamanho | Observação |
|---|---|---|
| `qwen2.5:7b` (padrão) | 4,7 GB | melhor português e melhor em seguir o formato |
| `llama3.2:3b` | 2,0 GB | mais rápido e leve; sugestões mais genéricas |

## Configuração

Arquivo: `%APPDATA%\com.buddy.desktop\config.toml` (criado na primeira execução,
com comentários). Linha apagada = valor padrão. Valores inválidos são corrigidos.

| Chave | Padrão | O que faz |
|---|---|---|
| `ollama_url` | `http://localhost:11434` | onde está o Ollama |
| `modelo` | `qwen2.5:7b` | modelo usado |
| `buscador` | `https://www.google.com/search` | busca do "Sim" (precisa ser https; texto vai em `?q=`) |
| `intervalo_entre_falas_seg` | 180 | mínimo entre falas automáticas (mín. 30) |
| `silencio_apos_recusa_min` | 60 | após "Agora não", não volta ao mesmo assunto |
| `estabilizar_seg` | 4 | contexto precisa ficar parado antes de pensar |
| `leitura_contexto_seg` | 2 | frequência de leitura da janela/mídia |
| `balao_seg` | 12 | tempo do balão na tela (pausa com o mouse em cima) |
| `apps_ignorados` | `[]` | `.exe` extras que o Buddy não deve ver |
| `titulos_ignorados` | `[]` | trechos de título extras (ex.: `"Projeto Secreto"`) |

### Privacidade

Além do que você puser na config, o Buddy **sempre** ignora (lista embutida em
`src-tauri/src/privacidade.rs`, que a config não consegue remover):

- **Apps:** WhatsApp, Discord, Telegram, Signal, Messenger, Slack, Teams,
  Outlook/Email/Thunderbird, KeePass, 1Password, Bitwarden.
- **Títulos** (para o que roda no navegador): WhatsApp/Discord/Telegram Web,
  Gmail, Outlook, Hotmail, bancos e pagamentos (Nubank, Itaú, Bradesco, BB,
  Santander, Caixa, Inter, C6, PicPay, Mercado Pago, PayPal...), gov.br,
  Receita, INSS, gerenciadores de senha e **abas anônimas/InPrivate**.

Numa janela privada, o título é descartado **no Rust** (nem chega ao front ou ao
modelo) e o Buddy não puxa assunto, nem sobre a música. Isso vale com ou sem
"não perturbe".

## Como testar

- **Console:** botão direito no Buddy → Inspecionar → Console. Mostra contexto,
  resposta crua do modelo, rejeições do zod e o motivo de ele *não* falar.
  Para ler o contexto na hora, digite `await buddyContexto()`.
- **Testes do Rust** (contexto real, allowlist, config):
  ```powershell
  cd src-tauri; cargo test -- --nocapture
  ```

---

## Arquitetura

```
                  ┌────────────────────── TypeScript (decide) ──────────────────────┐
 a cada 2s ──────▶│ contexto.ts  → mudou de verdade? → espera 4s parado             │
                  │ regras.ts    → não perturbe? falou há pouco? "Agora não"?       │
                  │ ia/prompt.ts → contexto vira DADO dentro de <contexto>          │
                  │ ia/esquema.ts→ zod valida a resposta (inválida = ignorada)      │
                  │ ui/balao.ts  → pergunta + 🔎 o que será feito → "Sim"?          │
                  └───────┬─────────────────────┬──────────────────────┬────────────┘
                   invoke │              invoke │               invoke │
                  ┌───────▼───────┐   ┌─────────▼────────┐   ┌─────────▼─────────┐
   Rust (executa) │ get_context   │   │ ask_llm          │   │ executar_acao     │
                  │ Win32 + GSMTC │   │ reqwest → Ollama │   │ enum fechado +    │
                  │ + privacidade │   │ (sem CORS)       │   │ URL montada aqui  │
                  └───────────────┘   └──────────────────┘   └───────────────────┘
                  config.rs (config.toml validado) · bandeja.rs (não perturbe, sair)
```

| Arquivo | Papel |
|---|---|
| `src/main.ts` | junta tudo: fluxo contexto → regras → modelo → balão → ação |
| `src/regras.ts` | "devo falar agora?" (puro, testável, sem Tauri) |
| `src/contexto.ts` | polling do contexto e detecção de mudança real |
| `src/ia/prompt.ts` | system prompt e montagem do contexto como dado |
| `src/ia/esquema.ts` | contrato da resposta (zod → também vira JSON Schema p/ Ollama) |
| `src/ia/cerebro.ts` | chama `ask_llm` / `executar_acao` |
| `src/ui/*` | personagem (SVG) e balão |
| `src/janela/*` | click-through e posição na tela |
| `src-tauri/src/contexto.rs` | janela ativa (Win32) + mídia (GSMTC) |
| `src-tauri/src/privacidade.rs` | o que o Buddy nunca vê (apps e títulos) |
| `src-tauri/src/ia.rs` | chamada ao Ollama |
| `src-tauri/src/acoes.rs` | allowlist de ações |
| `src-tauri/src/config.rs` | arquivo de config |
| `src-tauri/src/bandeja.rs` | ícone da bandeja |

### Segurança (prompt injection)

Títulos de janela são controlados por qualquer site, então são tratados como
**dado hostil**. Camadas:

1. Contexto vai como JSON dentro de `<contexto>`, com aviso explícito no prompt;
   `<` vira `\u003c` para o título não conseguir "fechar" a tag.
2. O Ollama recebe o JSON Schema da resposta (structured outputs).
3. zod valida tudo; qualquer desvio descarta a resposta inteira.
4. O Rust só aceita ações do `enum` e monta a URL ele mesmo (texto só no `?q=`).
5. O balão mostra exatamente o que o "Sim" fará, e nada roda sem esse clique.

O que realmente garante a segurança são as camadas 3, 4 e 5. A 1 ajuda, mas
nenhum modelo é imune a injection.

### Como ele decide o que dizer

O modelo recebe, além da janela e da música, uma **memória curta** (só em RAM,
sem janelas privadas, sem lista de títulos):

- horário e **período do dia** (entregue pronto: modelos pequenos erram ao
  deduzir "é madrugada?" a partir de "00:47");
- minutos seguidos **na janela** e **no mesmo programa**;
- **faixas seguidas** do mesmo artista e as últimas 5 faixas;
- programas mais usados nos últimos 30 min;
- **o que ele já disse** e sua reação (aceitou, recusou, ignorou).

**Marcos de tempo:** aos 45, 90, 150 e 240 min seguidos no mesmo programa, ele
ganha um motivo para falar mesmo sem nada mudar na tela (ex.: sugerir pausa de
madrugada). Ainda passa pelas regras de frequência.

**O código corrige o que o modelo erra** (vistos em teste, não hipotéticos):
- sugestão de algo que você **recusou/ignorou** nas últimas 2h → descartada,
  mesmo que o prompt já peça para não repetir;
- **oferta sem ação** ("quer ouvir...?" com só um botão Ok) → descartada;
- fala com **caracteres chineses/japoneses** (o qwen às vezes escorrega) → descartada;
- busca inventada em **marco de tempo** ("pausa para café") → ação removida;
- no **clique**, se a resposta for descartada, ele tenta mais uma vez.

Debug no console (modo dev): `buddyMemoria()` mostra exatamente o que vai ao modelo.

---

## Pontos fracos conhecidos

**Desempenho**
- ~240 MB de RAM, quase tudo do WebView2 (o Rust usa ~6 MB). É o preço de UI web.
- CPU parado ~0,5–0,9%. Janela transparente + qualquer animação = recomposição
  pela GPU a cada quadro; por isso ele fica parado em repouso.
- Polling (contexto a cada 2s, cursor a cada 40–150 ms) em vez de eventos do
  Windows (`SetWinEventHook`, eventos do GSMTC). Barato, mas não elegante.
- A primeira fala depois de um tempo parado demora (o Ollama descarrega o
  modelo após 10 min e leva ~30s para recarregar).

**Comportamento**
- Modelos pequenos ainda erram: às vezes contam errado ("quase 2h" com 35 min)
  ou inventam o que você está fazendo. O código filtra os erros mais comuns,
  não todos. A busca mostrada no balão (🔎) deixa você conferir antes do "Sim".
- A memória some ao fechar o app (de propósito, por privacidade), então o
  "Agora não" e o que ele já disse também.
- Com vários players abertos, o GSMTC pode reportar o que você não está ouvindo.
- Clicar no Buddy rouba o foco da janela em que você estava.
- A privacidade por título é por palavra-chave: um banco que não está na lista,
  ou um site que não põe o nome no título, passa. Revise a lista para o seu uso
  e acrescente em `titulos_ignorados`.
- Não inicia junto com o Windows.

**Técnicos**
- CSP desligada (`"csp": null`). Mitigado por sempre usar `textContent`, mas o
  certo é definir uma CSP restrita no `tauri.conf.json`.
- Monitores com escalas diferentes podem desalinhar a área clicável ao arrastar.
- Testes de TypeScript foram scripts avulsos; falta um runner (ex.: Vitest).
- Executável não assinado.

## Próximos passos

**Controle de mídia** (pedido): novas ações na allowlist, `proxima_faixa`,
`pausar` e `tocar`, usando o próprio GSMTC (`TrySkipNextAsync` etc.). Mesmo
modelo de segurança: `enum` no Rust + "Sim" obrigatório. Alternativa simples:
`buscador` apontando para o YouTube.

**Visão** (modelo que entende imagem):
1. Rust: comando `capturar_janela()` que tira print **só da janela ativa**
   (Windows.Graphics.Capture ou `PrintWindow`), reduz para ~768px e devolve PNG
   em base64. Respeita `apps_ignorados`. Nunca salva em disco.
2. `ask_llm` ganha um campo opcional `imagens: Vec<String>`, que vai no `images`
   da mensagem do Ollama. O resto do fluxo (zod, allowlist, "Sim") não muda.
3. Modelo: `qwen2.5vl:7b` ou `gemma3:4b` (bem mais pesados que os de texto).
4. Regras novas: visão só no **clique** ou com intervalo bem maior, porque é cara.
5. Atenção: texto **dentro da imagem** também pode ser prompt injection, e um
   print pode conter dados sensíveis. Opção na config para desligar a visão.
