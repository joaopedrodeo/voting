# Desenho técnico da primeira versão local

Estado: desenho para confirmação antes da implementação.

## Aplicação

Um servidor Rust renderiza as páginas HTML e expõe a busca pública. O navegador não precisa consultar diretamente as APIs legislativas. O servidor usa adaptadores separados para a Câmara dos Deputados e o Senado Federal e um arquivo SQLite local para o índice de busca, o cache e o estado de atualização.

## Caminho dos dados

1. Na primeira execução, o servidor consulta votações recentes e libera a página inicial quando elas estiverem disponíveis.
2. Em segundo plano, prepara o índice compacto de votações, propostas e parlamentares desde 2023. A interface mostra o progresso e a cobertura temporal já pronta.
3. Uma página de votação lê o índice e busca os votos completos na fonte oficial quando necessário. O servidor compartilha a resposta em cache por até 24 horas; uma cópia antiga pode ser mostrada com sua data se a fonte falhar.
4. A atualização diária cobre dados recentes. Uma revisão periódica cobre registros históricos e corrige o índice quando a fonte mudar. Ao reiniciar, a aplicação recupera atualizações pendentes.

## Integridade dos resultados

- Cada votação mantém sua identidade e link oficial. Votações de uma proposta podem aparecer juntas nas duas casas apenas quando o vínculo é verificável.
- Resultado, totais, votos individuais e categorias mantêm os valores oficiais. Divergências são mostradas com aviso.
- A lista de parlamentares usa o exercício do mandato na data da votação. Ausência na sessão requer confirmação oficial; falta de voto sem essa confirmação aparece como “sem voto registrado”. Se a composição não puder ser comprovada, a interface mostra a limitação.
- Busca por termo usa descrição e palavras-chave oficiais das propostas. Propostas sem votação nominal de Plenário ainda podem aparecer, com essa condição explícita.

## Critérios para a versão local

- Iniciar com `cargo run`, sem instalar banco separado, e oferecer contêiner como alternativa.
- Consultar votações nominais recentes de ambos os plenários e navegar até seus votos e fontes oficiais.
- Pesquisar propostas por número e termo oficial e filtrar por data, casa e parlamentar, inclusive votações em que este não registrou voto.
- Preencher o índice desde 2023 em segundo plano, mostrando progresso e resultados parciais claramente identificados.
- Mostrar dados antigos com data quando uma API falhar e avisar quando não houver dados disponíveis.
