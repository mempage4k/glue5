# Protocolo de identificação — GLUE5 (para chats de IA)

> Gravado em 2026-11-01 pelo chat `[MENU]`, a pedido do usuário. Reflete a
> convenção já em uso pelos chats ativos (`[MENU]` e `[PERF]`). Se o
> documento "Protocolo universal…" no Linear trouxer regras adicionais,
> elas prevalecem — atualize este arquivo.

Vários chats de IA (de plataformas/modelos diferentes) trabalham neste mesmo
projeto — mesmo Linear, mesmo repositório. Como tudo é postado pela conta do
mesmo usuário, a autoria real não aparece sozinha: ela precisa estar escrita
no próprio texto. Siga as regras abaixo em tudo o que você produzir.

## 1. Linha de identificação (obrigatória no Linear)

A PRIMEIRA linha de todo texto seu no Linear — descrição de issue,
comentário, documento, update de projeto — é a linha de identificação:

    [IA: <plataforma> | Agente/cliente: <agente> (<modo/contexto> — chat de <área>)]

Exemplo real:

    [IA: Arena.ai | Agente/cliente: Arena.ai (Agent Mode — chat de UI/menus)]

Se a sua plataforma não revela o modelo subjacente, identifique-se pela
plataforma/agente mesmo — o essencial é plataforma + qual chat/área você é.

## 2. Código de área (4 letras) — para títulos e textos curtos

Cada chat tem um código de 4 letras que identifica sua área. Ele prefixa
tudo que é curto e aparece em lista:

- Título de issue: `[PERF] ...`
- Título de commit: `[MENU] fix(ui): ...`
- Bullet no diário do projeto (`docs/project-diary.md`): `- [MENU] 2026-11-01 — ...`

Códigos já em uso:

- `MENU` — UI/menus do shell (Arena.ai)
- `PERF` — performance/Android

Se você é um chat novo: escolha um código de 4 letras que ainda não esteja
em uso, use sempre o mesmo, e anuncie a escolha (diário ou issue) para os
outros chats saberem quem é você.

## 3. Commits no GitHub

- Assunto do commit: `[<CÓDIGO>] <tipo>(<escopo>): <descrição curta>`
  Ex.: `[MENU] fix(ui): hint ancorado no canto superior direito`
- Corpo do commit (quando relevante): contexto/causa-raiz, o que mudou e
  qual validação foi feita — ou a ausência dela, honestamente
  (ex.: "não compilado; aguardando build do usuário").
- Trabalhe e faça push SOMENTE na sua branch; nunca direto na `main`.

## 4. Preservação de autoria e histórico

- NUNCA edite nem apague texto de outro chat (Linear ou git). Se algo
  estiver errado, corrija com um comentário/bullet novo, identificado
  como seu.
- Ao referenciar trabalho alheio, cite o código dele:
  "o chat [PERF] registrou que…".
- Correções retroativas de identificação: só nos seus próprios textos.

## 5. Coordenação entre chats

- Arquivos tocados por mais de um chat: um de cada vez commita; o outro
  faz rebase sobre o estado novo. Avise na issue de coordenação antes de
  editar arquivo compartilhado.
- Não mexa em arquivos de outra área sem avisar antes
  (ex.: [PERF] não edita `ui/**` sem aviso; [MENU] não mexe no runtime).

Resumo: primeira linha você se identifica, título curto você usa seu
código, e o texto dos outros você nunca altera.
