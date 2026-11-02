# Protocolo de identificação e colaboração — GLUE5

[IA: não identificado | Agente/cliente: Arena.ai (Agent Mode — chat de UI/menus)]

> Este arquivo espelha, **verbatim**, os dois documentos oficiais de
> colaboração do workspace Glue5 (Partes A e B), colados pelo usuário em
> 2026-11-01. Motivo do espelho: os originais vivem como **documentos do
> Linear**, que IAs não conseguem ler via MCP (login wall) — aqui no
> repositório, qualquer chat os lê antes de escrever. A **Parte C** é um
> adendo local (autoria do chat `[MENU]`) com as convenções práticas que
> os chats ativos já combinam e que não estão nos documentos oficiais; em
> caso de conflito, **as Partes A e B prevalecem**. Se o usuário atualizar
> os originais no Linear, atualize este espelho.

---

# Parte A — Documento oficial: Protocolo universal (Linear)

# PROTOCOLO UNIVERSAL — Identificação e colaboração entre IAs

# Protocolo universal de colaboração entre IAs — Linear

> **Regra de identificação:** toda IA que criar ou alterar conteúdo neste workspace deve identificar a própria contribuição de forma explícita. Esta regra vale para qualquer modelo, fornecedor, cliente, integração ou agente.

## 1. Formato obrigatório de identificação

Use a primeira linha de cada contribuição escrita:

`[IA: <nome do modelo> | Agente/cliente: <nome da ferramenta ou integração>]`

Exemplos:

* `[IA: GPT-6 | Agente/cliente: ChatGPT]`
* `[IA: Claude Sonnet | Agente/cliente: Claude Code]`
* `[IA: Gemini | Agente/cliente: Gemini CLI]`
* `[IA: não identificada | Agente/cliente: <ferramenta>]`

Não invente nem presuma o modelo. Se não souber o modelo exato, escreva `não identificado`. Se souber o modelo mas não o cliente, identifique o que for conhecido.

## 2. Onde aplicar

A identificação deve aparecer em toda contribuição textual criada ou modificada pela IA:

* comentários em issues, projetos, iniciativas e documentos;
* novas issues e descrições de issues;
* atualizações de status e relatórios de projeto/iniciativa;
* documentos novos ou editados;
* notas, relatórios, decisões técnicas e registros de validação.

Em comentários e atualizações independentes, a identificação deve ser a primeira linha.

Ao criar uma issue ou documento, coloque a identificação no início da descrição/conteúdo. Ao editar conteúdo existente, **não apague nem reescreva a autoria anterior**: acrescente a contribuição identificada ao final, ou registre-a em um comentário identificado. Para documentos de referência, mantenha um pequeno histórico de contribuições quando fizer sentido.

Títulos e nomes de issues/projetos devem permanecer limpos; não coloque o nome da IA no título. Não altere o campo de autor/creator nem tente se passar por uma pessoa.

## 3. Registro de alterações

Quando uma IA mudar status, prioridade, responsável, labels, datas, relações, projeto ou outros campos sem um espaço próprio para identificação, ela deve deixar um comentário na mesma entidade explicando a alteração, com a identificação na primeira linha.

O comentário deve informar:

* o que foi alterado;
* por que foi alterado;
* quais fatos ou verificações sustentam a decisão;
* o que continua pendente, quando aplicável.

Não crie comentários artificiais para atualizações triviais em lote; agrupe alterações relacionadas em um único comentário por entidade, sempre identificando a IA.

## 4. Colaboração e preservação do trabalho

* Antes de agir, leia este protocolo e consulte o histórico da issue/projeto/documento relevante.
* Verifique se outra IA ou pessoa já está trabalhando no mesmo item. Não duplique tarefas ou decisões.
* Preserve comentários, decisões, autoria, evidências e histórico anteriores.
* Não marque trabalho como concluído sem verificar os critérios de aceite. Diferencie claramente concluído, parcialmente concluído, bloqueado e aguardando ação humana.
* Não atribua ações a outra IA ou pessoa sem evidência.
* Se encontrar informação contraditória, registre a divergência e a fonte; não apague silenciosamente a informação anterior.
* Não exponha segredos, tokens, senhas, dados privados ou arquivos protegidos.
* Se não puder cumprir uma parte destas regras por limitação da integração, declare a limitação no registro.

## 5. Formato sugerido para comentários

`[IA: <modelo> | Agente/cliente: <ferramenta>]`

`**Ação:** <o que foi feito>`

`**Motivo/evidência:** <por que e como foi verificado>`

`**Pendências:** <o que falta, ou “nenhuma”>`

## 6. Limite importante

Este documento é a convenção oficial do workspace Glue5. Ele orienta agentes que tenham acesso ao documento e o leiam, mas o Linear não consegue obrigar automaticamente todos os modelos, clientes externos ou integrações a respeitá-lo. Cada agente deve procurar e ler este protocolo antes de escrever no workspace. Se uma ferramenta oferecer instruções globais/de sistema próprias, o agente deve também registrar esta convenção nelas quando autorizado.

**Idioma padrão do workspace:** português (Brasil). Preserve nomes de código, identificadores, mensagens técnicas e termos oficiais no idioma original quando necessário.

---

# Parte B — Documento oficial: Introdução — commits e codinome

# INTRODUÇÃO — Identificação de commits e codinome do GLUE 5

[IA: GPT-6 | Agente/cliente: ChatGPT]

# Introdução — identificação de commits e codinome do GLUE 5

Este documento registra duas convenções permanentes do projeto: como identificar contribuições de inteligência artificial nos commits e como administrar o codinome interno do GLUE 5. Estas regras complementam o **Protocolo universal de identificação e colaboração entre IAs** já mantido neste workspace.

## 1. Identificação obrigatória em commits com contribuição de IA

Todo commit que contenha alterações produzidas ou materialmente modificadas por uma IA deve identificar a contribuição. Isso inclui código, documentação, scripts, configurações, testes e outros arquivos do projeto.

A identificação deve informar, no mínimo:

* **Modelo:** nome real do modelo utilizado (por exemplo, GPT-6, Claude, Gemini ou Qwen).
* **Agente/cliente:** ferramenta ou agente que executou o trabalho (por exemplo, ChatGPT, OpenCode, Cursor ou outro cliente).

### Formato recomendado

Preserve o assunto técnico do commit na primeira linha. Registre a identificação no corpo do commit usando este trailer:

`AI-Contribution: <modelo> | Agente/cliente: <ferramenta>`

Exemplo:

```text
feat(android): ajustar o layout do HUD

AI-Contribution: GPT-6 | Agente/cliente: ChatGPT
```

Quando mais de uma IA tiver contribuído materialmente, inclua um trailer para cada combinação de modelo e agente/cliente. Use nomes específicos e verdadeiros; não escreva apenas "IA", "bot" ou "assistente".

### Quando aplicar

* **Obrigatório:** a IA escreveu, gerou ou alterou materialmente parte das mudanças incluídas no commit, mesmo que uma pessoa revise, complemente ou faça o push.
* **Não obrigatório:** a IA apenas analisou, explicou ou revisou o trabalho, sem produzir alterações incorporadas ao commit.
* **Commits exclusivamente humanos:** não precisam do trailer de contribuição de IA.

Não atribua a autoria humana a uma IA nem o contrário. Preserve os campos normais de autor e committer do Git; o trailer serve para registrar a participação da IA, não para substituir a autoria do Git.

Esta convenção específica para commits mantém a primeira linha livre para o resumo técnico. Nas demais contribuições textuais feitas no Linear, continue seguindo a identificação na primeira linha exigida pelo protocolo universal.

## 2. Codinome do projeto

O codinome atual do GLUE 5 é `willow`.

O codinome identifica uma fase ou identidade importante do projeto. Ele **não representa a versão do aplicativo, seu estágio de desenvolvimento nem seu estado de publicação**.

### Regra de alteração

Altere o codinome somente quando houver uma mudança grande ou significativa no projeto, por exemplo:

* mudança relevante na arquitetura ou na organização fundamental do sistema;
* reformulação ampla da experiência ou da estrutura do aplicativo;
* substituição importante de uma abordagem técnica central;
* mudança de direção ou início de uma nova fase estrutural do projeto.

Correções de bugs, ajustes pontuais, pequenas refatorações, novas funcionalidades isoladas e atualizações rotineiras **não** justificam trocar o codinome.

### Formato obrigatório

Todo codinome novo deve:

* ser **uma única palavra**;
* ser simples, distinto e fácil de escrever;
* não conter espaços, números ou combinações de palavras;
* não ser um termo que indique estado ou etapa de versão.

Não use termos como `release`, `beta`, `alpha`, `stable`, `preview` ou equivalentes. Esses termos podem ser usados para indicar o estado de uma versão, mas nunca como codinome.

### Separação entre codinome e versão

Mantenha os conceitos independentes:

* **Codinome:** identidade interna da fase significativa do projeto (atualmente, `willow`).
* **Versão:** número ou identificador da versão do software.
* **Estado da versão:** situação de desenvolvimento ou distribuição, como alpha, beta, release candidate ou estável.

Uma mudança no estado ou no número da versão não exige, por si só, mudança do codinome.

### Histórico

Ao mudar o codinome, atualize este documento e os locais relevantes de referência do projeto. Registre a data, o codinome anterior, o novo codinome e uma justificativa breve para a mudança. Não altere retroativamente o histórico de versões.

Até que ocorra uma mudança significativa conforme os critérios acima, mantenha `willow` como codinome vigente.

## 3. Aplicação por agentes e colaboradores

Todos os agentes e colaboradores devem consultar estas convenções antes de criar commits ou atualizar referências ao codinome. Em caso de dúvida sobre a dimensão de uma mudança, mantenha o codinome atual e registre a dúvida para avaliação; não invente uma troca apenas para nomear uma atualização rotineira.

Este documento complementa o protocolo universal do workspace e deve ser atualizado quando estas convenções forem formalmente alteradas.

## Atualização — codename curto (2026-11-01)

Além de ser uma única palavra em inglês, o codename deve ser curto e fácil de escrever; prefira aproximadamente 4–8 letras. Não use espaços, números ou termos de estado de versão (como `release`, `beta`, `alpha`, `stable` ou `preview`). O codename atual continua sendo `willow`.

---

# Parte C — Adendo local: convenções práticas entre os chats

> Autoria: chat `[MENU]` (2026-11-01). Isto **não** faz parte dos
> documentos oficiais acima — só registra o que os chats ativos combinam
> na prática e que lá não aparece. Em conflito, valem as Partes A/B.

## C.1 Códigos de área (4 letras)

Cada chat usa um código de 4 letras para marcar a **área** do trabalho em
textos curtos e em lista (assunto de commit, título de issue, bullet do
diário). Códigos em uso:

- `MENU` — UI/menus do shell (chat Arena.ai)
- `PERF` — performance/Android (outro chat)

Chat novo: escolha um código de 4 letras livre, use sempre o mesmo e
anuncie a escolha (diário ou issue de coordenação).

## C.2 Como as camadas se compõem

- **Identidade da IA** (modelo + agente/cliente — o que os documentos
  oficiais exigem): 1ª linha de todo texto no Linear; trailer
  `AI-Contribution:` no corpo do commit.
- **Código de área** (`[MENU]`, `[PERF]`…): **não** identifica a IA —
  marca a área, porque vários chats podem usar a mesma plataforma. Ele
  aparece como prefixo curto; a identidade formal continua sendo a
  linha/trailer.
- A Parte A pede títulos limpos (sem nome da IA). Leitura adotada: o
  código de área não é nome de IA, então pode prefixar; se o usuário
  preferir a leitura estrita, os códigos saem dos títulos e ficam só no
  diário e nos corpos. *(Decisão sinalizada ao usuário em 2026-11-01,
  aguardando confirmação.)*

Exemplo completo de commit:

```text
[MENU] fix(ui): hint ancorado no canto superior direito

<corpo: contexto, causa-raiz, validação feita ou pendente>

AI-Contribution: não identificado | Agente/cliente: Arena.ai (Agent Mode — chat de UI/menus)
```

## C.3 Diário do projeto

Entradas de cada chat são bullets tagueados com o seu código:

```text
- [MENU] Adendo (2026-11-01): …
```

Assim o histórico fica legível por área; bullets antigos sem tag ficam
como estão — não reescreva o diário alheio.

## C.4 Branches e arquivos compartilhados

- Cada chat trabalha e faz push **somente na própria branch**; nunca
  direto na `main`.
- Arquivos tocados por mais de um chat (ex.: `app.rs`, `ui.rs`,
  `ui/pages.rs`): um de cada vez commita e o outro faz rebase sobre o
  estado novo; avise na issue de coordenação (hoje, GLU-26) antes de
  edições cirúrgicas.
- Não mexa em arquivos de outra área sem avisar antes (ex.: `[PERF]`
  não edita `ui/**` sem aviso; `[MENU]` não mexe no runtime).

## C.5 Chats na Arena.ai

A Arena.ai não divulga o modelo subjacente ao agente. Pela regra da
Parte A ("não invente nem presuma o modelo"), chats da Arena escrevem
`não identificado` no campo do modelo e se distinguem pelo
agente/cliente + área — ex.:
`[IA: não identificado | Agente/cliente: Arena.ai (Agent Mode — chat de UI/menus)]`.
