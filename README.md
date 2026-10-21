# Votações legislativas federais

Aplicação local para consultar votações nominais concluídas a partir de um índice SQLite.

## Iniciar

Com Rust instalado, execute:

```sh
cargo run
```

Abra <http://127.0.0.1:3000/>. O arquivo `voting.sqlite3` é criado no diretório atual e permanece entre execuções. Ao iniciar, o servidor consulta em segundo plano as votações nominais recentes dos Plenários da Câmara e do Senado (últimos 30 dias, a partir de 2023). Elas aparecem juntas na página inicial em ordem de data, assim que são importadas. Uma falha de qualquer fonte não remove votações já indexadas. O histórico completo será implementado nos próximos tickets.

A coleta do Senado usa a [API oficial de votações](https://legis.senado.leg.br/dadosabertos/api-docs/swagger-ui/index.html), em intervalos curtos para respeitar o limite de requisições. O link “Fonte oficial” de cada votação do Senado abre a resposta da sessão na API; a identificação da votação aparece na página de detalhe. Os totais oficiais só aparecem quando publicados pela fonte: a API documenta `totalVotosSim`, `totalVotosNao` e `totalVotosAbstencao` como totais de votações secretas, e normalmente não os fornece para as votações públicas indexadas.

[A Câmara não publica um indicador estruturado confiável da modalidade da votação](https://dadosabertos.camara.leg.br/howtouse/2020-02-07-dados-votacoes.html). Quando os dados oficiais não permitem distinguir uma votação nominal de uma simbólica com registros individuais parciais, o servidor não a inclui no índice.

Para alterar os caminhos padrão:

```sh
VOTING_DB_PATH=/caminho/para/indice.sqlite3 VOTING_LISTEN_ADDR=127.0.0.1:3001 cargo run
```

O diretório do arquivo SQLite precisa existir. O servidor usa o mesmo arquivo em todas as execuções quando `VOTING_DB_PATH` permanece igual.
