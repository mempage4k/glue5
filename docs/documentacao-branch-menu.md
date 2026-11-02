# Documentação da branch [MENU] — índice e decisão

[IA: não identificado | Agente/cliente: Arena.ai (Agent Mode — chat de UI/menus)]

Índice explícito da documentação produzida pela área de UI/menus que hoje
existe **só nesta branch** (`arena/4f5cc306-glue5`), criado na
regularização de 2026-11-01 pedida pela revisão geral. Medido sobre o
HEAD `a2e5bfe` (22 commits à frente e 1 atrás de `origin/main` =
`af1158b`).

## Decisão registrada

A camada documental **permanece na branch do especialista** até o dono
aprovar a publicação em `main` (integração centralizada, conforme o
`AI_GUIDE.md`). Enquanto isso, este arquivo é o índice de referência:
qualquer issue que cite um destes docs só é verificável nesta branch.

A seção correspondente no `docs/README.md` ("Documentação em branches de
especialistas") fica a cargo da revisão geral — **autorizada** por esta
área, com o pedido de avisar na GLU-26 o que mudou (+commit) para os
especialistas realinharem. Eu não edito `docs/README.md` nem
`CONTRIBUTING.md` nesta regularização, para não conflitar com a edição
anunciada dela.

## Os 7 documentos (só nesta branch)

| Documento | Branch | Último commit | Status |
|---|---|---|---|
| `docs/gta5-reference-inventory.md` | `arena/4f5cc306-glue5` | `8343b5e` (2026-11-01) | Ativo — fonte de verdade da referência GTA V (citado pelas issues) |
| `docs/gta5-visual-parity.md` | idem | `50872c2` (2026-11-01) | Ativo — auditoria de paridade (base de GLU-10, GLU-13…GLU-19) |
| `docs/font-extraction.md` | idem | `6aeb8b9` (2026-11-01) | Ativo — desenho (GLU-24; implementação pendente) |
| `docs/rpf-awc-reader.md` | idem | `176e089` (2026-11-01) | Pesquisa (GLU-21; implementação de outra área) |
| `docs/protocolo-identificacao.md` | idem | `a2e5bfe` (2026-11-01) | Vigente — espelho dos documentos oficiais (Partes A/B verbatim) + adendo local |
| `docs/shell-notifications.md` | idem | `683385a` (2026-11-01) | Desenho implementado (GLU-23; roadmap no próprio doc) |
| `docs/prompt-chatgpt-2026-11-01-ysc-full.md` | idem | `4b02761` (2026-11-01) | Histórico — rodada 2 concluída; registro |

Companion de dados do inventário, também só nesta branch:
`docs/gta5-referencia/` (7 CSVs de metadados/hashes; último commit
`8343b5e`). Nenhum asset proprietário — só metadados.

## Relacionados (existem em `main`, versão corrente só aqui)

- `docs/menu-sfx.md` — existe em `main` numa versão anterior; a versão
  com a lógica verificada dos sons de frontend está nesta branch.
- `docs/project-diary.md` — as duas linhas divergiram (conflito
  pré-existente conhecido na integração; resolver mantendo os bullets
  `[MENU]`/`[PERF]`/main).

## Recomendação sobre o protocolo (resposta (d) à revisão geral)

`docs/protocolo-identificacao.md` deve ir para `main` **agora**, sem
esperar o build Android: é documentação pura (zero impacto de build) e é
hoje o único espelho das regras de colaboração legível por IAs (os
originais do Linear têm login wall via MCP). Cada chat novo que entra
sem lê-lo trabalha às cegas. A publicação em si (PR/merge) é do fluxo
dono → coordenador; esta área não a executa.

---
*Histórico de contribuições deste arquivo: criado por [MENU] em
2026-11-01 na regularização; atualizar a tabela quando um doc for
publicado em `main` (mover a linha para "publicados").*
