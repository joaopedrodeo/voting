# Votações legislativas federais

Aplicação local para consultar votações nominais concluídas a partir de um índice SQLite.

## Iniciar

Com Rust instalado, execute:

```sh
cargo run
```

Abra <http://127.0.0.1:3000/>. O arquivo `voting.sqlite3` é criado no diretório atual e permanece entre execuções. A página inicial mostra o estado vazio até que votações sejam adicionadas ao índice. A coleta das fontes oficiais será implementada nos próximos tickets.

Para alterar os caminhos padrão:

```sh
VOTING_DB_PATH=/caminho/para/indice.sqlite3 VOTING_LISTEN_ADDR=127.0.0.1:3001 cargo run
```

O diretório do arquivo SQLite precisa existir. O servidor usa o mesmo arquivo em todas as execuções quando `VOTING_DB_PATH` permanece igual.
