# Estado de integração entre branches

`[IA: não identificado | Agente/cliente: Arena.ai (Agent Mode — chat de revisão geral)]`

Retrato em **2026-11-01** (hora local do dono). Finalidade: um único mapa de
*qual branch tem o quê*, *quem é dono de qual arquivo*, *quais conflitos já
são conhecidos* e *qual é o gate*. **Não é diário**, não é autorização de
publicação e não substitui o project update do Linear.

Base: `main` / `origin/main` = `af1158b`. Repositório: `luizhegner/glue5`.

## 1. Branches

| Área | Branch | Ponta | Publicação | Estado do trabalho |
|---|---|---|---|---|
| Base | `main` | `af1158b` | publicada | PR #12 mira aqui |
| `[TOUCH]` | `arena/107599ac-glue5` | PR #12 (11 à frente / 0 atrás) | publicada; **PR #12 aberto**, 3 checks verdes | Edição de `docs/roadmap-touch-hud-benchmarks.md` **não commitada** |
| `[MENU]` | `arena/4f5cc306-glue5` | remota `a2e5bfe`; local `84befb0` | remota publicada; `84befb0` **local, sem push** | 7 docs + 7 CSVs branch-only; índice em `docs/documentacao-branch-menu.md` |
| `[SERV]` | `arena/57946d80-glue5` | commit local `313075f` | **ausente do remote** | 2 docs + `docs/README.md` + diário commitados localmente, **sem push** |
| `[PERF]` | `arena/b8d177aa-glue5` (citada na GLU-26) | 9 commits **recriados** | **ausente do remote** | 28 arquivos, +1251/−24; o sandbox re-clonou e apagou os commits originais, que foram refeitos com trailer `AI-Contribution:`; **os SHAs mudaram** |
| Revisão geral | `arena/f2f782ce-glue5` | commits locais `6f6018d` + esta rodada | **sem push** | Documentação/governança transversal apenas |

`git ls-remote --heads origin` confirma que `arena/57946d80-glue5` e
`arena/b8d177aa-glue5` **não existem no remoto**. A GLU-26 diz "Já no
repositório (branch `arena/b8d177aa-glue5`)" — essa frase é imprecisa e deve
ser lida como "trabalho local". Correção registrada em comentário identificado
na GLU-26, sem reescrever a descrição alheia.

Divergência de medição registrada sem presumir erro: a revisão geral mediu
~21 à frente / 9 atrás para `[MENU]`; o `[MENU]` mediu 22/1 (e 23/1 após o
commit local). Momentos diferentes e chats ativos.

## 2. Ownership de arquivos compartilhados

| Arquivo | Situação confirmada |
|---|---|
| `crates/glue5/src/app.rs` | Compartilhado `[MENU]` (2 commits) × `[PERF]` |
| `crates/glue5/src/ui.rs` | Compartilhado `[MENU]` (5 commits) × `[PERF]` |
| `crates/glue5/src/ui/pages.rs` | Compartilhado `[MENU]` (1) × `[PERF]`; superfície também de `[TOUCH]` e `[SERV]` |
| `crates/glue5/src/settings.rs` | Compartilhado; **0 commits do `[MENU]`**; `[TOUCH]` não alterou nesta rodada |
| `crates/glue5/src/imports.rs` | Compartilhado; **0 commits do `[MENU]`** |
| `crates/glue5-runtime/src/input.rs` | Runtime compartilhado, segundo `[TOUCH]` |
| `crates/glue5/src/ui/popup.rs` | **Exclusivo `[MENU]`** — único arquivo com ownership não compartilhado |
| `docs/README.md` | Três autores (`main`, `[SERV]`, revisão geral) + versão do PR #12 |
| `docs/project-diary.md` | `[MENU]`, `[PERF]`, `[SERV]` e `main` — append × append |
| `docs/menu-sfx.md` | Existe em `main` e na `[MENU]` em versões divergentes |

O `[PERF]` confirmou que **nenhum dos cinco arquivos é exclusivamente seu** e
reproduziu a medição da revisão geral (2/5/1/0/0 para `[MENU]`). Acrescentou
um dado que faltava: a branch `[TOUCH]` (`arena/107599ac-glue5`) mexe nos
**cinco** arquivos e é quem mais altera `settings.rs` e `imports.rs`. O
`[SERV]` avisou na GLU-26 antes de editar `docs/README.md` e o diário.

## 3. Conflitos conhecidos

1. **`docs/project-diary.md`** — append × append, pré-existente. O `[PERF]`
   refez o merge em worktree com o tip `e0c4963` e confirmou resolução trivial,
   preservando `main` + `[MENU]` + `[PERF]`. Os três arquivos Rust
   auto-mergeiam (`watch_heat`/Thermal back-off de um lado,
   `Ui::alert()`/`Hint`/`popup::ask` do outro).
2. **`docs/README.md`** — quatro versões em circulação. Requer ordem de
   integração explícita; este é o conflito novo introduzido pela delegação.
3. **`docs/menu-sfx.md` / `docs/project-diary.md`** — versões divergentes entre
   `main` e `[MENU]`; ver índice da `[MENU]` antes de mesclar.
4. **`docs/streaming-flow.md` / `docs/windows-stream-audio-audit.md`** — novos,
   apenas no worktree do `[SERV]`, ainda sem commit.

## 4. CI e evidência

Ver [`ci-coverage.md`](ci-coverage.md). Resumo: nenhum workflow roda
`cargo test --workspace`; `import-tests.yml` só dispara em `main`/PRs para
`main`; `gpu-tests.yml` roda apenas `cargo test -p glue5-gpu`. O filtro `ui::`
**cobre** `ui::popup::tests::*`, mas nunca executou na `[MENU]` — a GLU-12
segue *In Review* por falta de run, não por falta de teste.

`workflow_dispatch` do import-tests é um pipeline Android de 120 min com ntfy
e `contents: write`: reservado ao dono por `AI_GUIDE.md`. Nenhum agente deve
dispará-lo.

## 5. Gate e ordem de integração sugerida (não executada)

**Gate único: o Build Android do dono.** Sem ele, nada se publica nem se
integra.

1. Dono roda o build e reporta: saída da compilação, linhas `thermal:`,
   benchmark antes/depois, e — se a build expuser as opções — o A/B do
   celular com o trace `Recent phone movie methods`.
2. Cada chat **commita localmente** (sem push) o que hoje está só no worktree:
   `[TOUCH]` (roadmap), `[SERV]` (2 docs + README + diário), `[PERF]`
   (9 commits já locais).
3. Dono autoriza explicitamente a publicação das branches `[PERF]` e `[SERV]`
   e do `84befb0` da `[MENU]`.
4. Coordenador integra na ordem: `main` ← PR #12 (`[TOUCH]`) ← `[MENU]` ←
   `[PERF]` ← `[SERV]` ← revisão geral, resolvendo diário e README uma vez,
   com aviso na GLU-26.
5. Só então: `mise run test` / `cargo test --workspace` e um PR para `main`
   que dispare `import-tests.yml`.

Nenhum passo acima foi executado. **Não há push, PR, merge nem workflow
disparado por esta revisão.**

## 6. Riscos operacionais

- **Já aconteceu:** o sandbox do `[PERF]` re-clonou o repositório e apagou 9
  commits; só o working tree sobreviveu. Os commits foram recriados com o mesmo
  diff, mas os SHAs mudaram. **Commits locais não são armazenamento seguro** —
  a publicação (autorizada pelo dono) é o que protege o trabalho.
- Trabalho não commitado do `[TOUCH]` pode se perder entre sessões do sandbox.
- O `[MENU]` informou ao dono que normalmente publica a cada entrega, mas
  deixou `84befb0` local por causa da instrução de não-fazer-push.
- Três formatos de identificação de IA circulam; dois colocam `Arena.ai` no
  campo do **modelo**, violando a Parte A. Ver o apêndice em
  [`protocolo-identificacao.md`](protocolo-identificacao.md).
- O conector Linear não cria milestones nem cycles; responsáveis, due dates e
  estimativas seguem sem definição por decisão do dono.

## 7. O que este documento não é

Não é autorização de publicação, não é registro cronológico (isso é o
`project-diary.md`), não é fonte de verdade de backlog (isso é o Linear) e não
substitui as descrições das issues. É apenas o mapa para o coordenador.
