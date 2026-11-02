# Identidade de build — qual APK é qual

`[IA: não identificado | Agente/cliente: Arena.ai (Agent Mode — chat de revisão geral)]`

Retrato de **2026-11-02**. Finalidade: acabar com a confusão entre os APKs das
branches e com a dúvida "essa alteração está no meu repo?". Não é diário, não é
autorização de publicação e não substitui o Linear.

## Causa raiz

* Todas as branches produziam **`Glue5-android.apk`**, com o mesmo nome.
* Todas enviavam o arquivo para o **mesmo draft release** (`game`).
* `.github/scripts/draft.sh upload` **apaga o asset de mesmo nome** antes de subir
  o novo. Ou seja, o APK da área anterior não ficava "com nome repetido": ele era
  **substituído e perdido** a cada build.
* `concurrency: group: build-${{ inputs.platform }}` colocava builds de branches
  diferentes na mesma fila — uma podia cancelar a outra.
* `AI_GUIDE.md` e `docs/linear-workflow.md` já pedem que um chat diga qual
  branch/commit contém o trabalho e se ele foi publicado, mas isso dependia de
  cada chat lembrar. Nada no arquivo baixado identificava a origem.

## Padrão adotado neste branch

`Glue5-<plataforma>-<branch>-<sha7>.<ext>`, por exemplo:

```
Glue5-android-arena-107599ac-glue5-8767ebd.apk
Glue5-BUILD-arena-107599ac-glue5-8767ebd.txt
```

O `.txt` vai junto para o draft e registra branch, commit completo, assunto do
commit, plataforma, hora UTC e a URL do run do Actions. O **keystore continua
`Glue5-android.keystore`**, de propósito: a chave de assinatura é a mesma para
todos os builds e precisa ser reutilizada. O `concurrency` passa a incluir
`github.ref`. O label do asset no GitHub já mostrava o SHA curto; agora o nome
do arquivo também mostra.

**Regra para o dono:** o nome do APK diz de qual branch e de qual commit ele
veio. Se não bater com o que o chat informou, **não é o build certo — não
instale**. O draft passa a acumular um APK por build em vez de apagar o
anterior; apague os antigos manualmente quando quiser.

## Regra de comunicação entre chats

Antes de sugerir qualquer build, todo chat deve informar na própria resposta:

1. a **branch**;
2. o **SHA do commit**;
3. se está **publicado (push)** ou **apenas local** — e, nesse caso, escrever
   explicitamente que **não está em `main` nem no repositório do dono**.

Um commit local não aparece no GitHub Actions e não gera APK. Na dúvida,
conferir `git ls-remote --heads origin` ou o cabeçalho do run do Actions.

## Nas outras branches

`workflow_dispatch` executa o `.github/workflows/build.yml` **da branch
selecionada**. A correção deste branch não vale para os builds das outras
áreas: cada chat precisa aplicar o mesmo padrão no seu próprio workflow.
Modelo, de dentro de qualquer checkout:

```
git fetch origin arena/f2f782ce-glue5
git show FETCH_HEAD:.github/workflows/build.yml
```

Há uma mensagem pronta, entregue ao dono fora do repositório, para repassar
aos chats.

## Estado

* **Implementado neste branch:** sim — `.github/workflows/build.yml`,
  `docs/getting-started.md`, `docs/github-build.md` e este documento.
* **Publicado:** `575f816` em `arena/f2f782ce-glue5` (push autorizado pelo
  dono em 2026-11-02). Sem PR, merge, integração em `main` nem workflow
  disparado.
* **Não testado em build real:** nenhum Build Android foi executado para
  validar o novo nome. O YAML e todos os blocos `run:` foram validados
  estaticamente; a validação de verdade é o próximo Build do dono no Actions.
* Mudança pequena e reversível. `0.1.0`, `willow` e `VERSION_CODE` intactos.
* `docs/README.md` não foi alterado para não agravar o conflito de integração
  conhecido (quatro versões em circulação).
