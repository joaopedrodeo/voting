# Consulta pública de votações federais

Estado: decisões iniciais; desenho em andamento.

## Objetivo

Permitir que qualquer pessoa consulte votações legislativas federais concluídas e veja como cada parlamentar votou, inclusive respostas além de “sim” e “não”.

## Escopo acordado

- Começar por assuntos federais em Brasília, usando dados da Câmara dos Deputados e do Senado Federal.
- Cobrir inicialmente as votações nominais dos Plenários da Câmara dos Deputados e do Senado Federal. Comissões e sessões conjuntas ficam para uma etapa posterior.
- Incluir inicialmente apenas votações nominais com registros individuais verificáveis.
- Oferecer votações desde 2023 e as recentes. Ampliar o histórico gradualmente e atualizar o índice de busca periodicamente, com atualização diária como meta inicial.
- Oferecer consulta pública, busca por votação e filtros por data, casa legislativa, proposta e parlamentar, sem contas ou alertas pessoais no primeiro lançamento.
- Apresentar na página inicial as votações em ordem de data, com acesso à proposta relacionada.
- Carregar inicialmente as votações recentes; oferecer filtros e paginação para acessar datas desde 2023.
- Oferecer uma página para cada votação e outra para a proposta relacionada, reunindo as votações dessa proposta.
- Reunir votações da Câmara e do Senado na mesma página de proposta somente quando o vínculo for confirmado por dados oficiais. Manter registros separados quando não houver confirmação.
- Mostrar em cada votação o item votado, a data, o resultado, o link para a fonte oficial e os votos individuais.
- Mostrar o resultado e os totais oficiais, além da data da última atualização dos dados. Refletir correções feitas nas fontes oficiais.
- Se os totais oficiais divergirem da lista individual, mostrar ambos com aviso, sem alterar os números publicados.
- Usar a descrição oficial do item votado, sem resumo produzido pelo site no primeiro lançamento.
- Permitir busca de propostas por número, descrição e palavras-chave oficiais, inclusive termos como “escala 6x1” quando presentes nesses dados. Mostrar todas as propostas correspondentes; apelidos sem suporte nos dados oficiais podem ser incorporados posteriormente com fonte identificada.
- Mostrar propostas correspondentes mesmo quando ainda não tenham votação nominal de Plenário, com essa condição explícita.
- Preservar as categorias de voto publicadas pela fonte oficial, com um filtro agrupado em sim, não e outros.
- Listar os parlamentares em exercício na data de cada votação, incluindo os que não têm voto registrado, e mostrar o partido correspondente àquela data.
- Se a composição em uma data não puder ser verificada, avisar que a lista pode estar incompleta e não atribuir “sem voto registrado” a pessoas cuja participação no mandato seja incerta.
- Ao filtrar por parlamentar, mostrar todas as votações ocorridas durante seu exercício do mandato, inclusive aquelas sem voto registrado, com o estado individual em cada resultado.
- Exibir “ausente na sessão” quando uma fonte oficial confirmar essa ausência. Caso contrário, a falta de registro individual deve aparecer como “sem voto registrado”.
- Aceitar cobertura parcial de ausências confirmadas no primeiro lançamento, deixando clara a indisponibilidade de confirmação quando necessário.
- Concluir primeiro uma versão que funcione localmente. Depois, publicar na internet com meta de hospedagem de até US$ 15 por mês, sem contar domínio; o homelab é uma opção.
- Usar Rust no servidor para gerar as páginas HTML com pouco JavaScript. Go permanece como alternativa se surgir um impedimento concreto.
- Manter um índice local compacto de propostas, votações e vínculos com parlamentares para busca histórica. Consultar votos completos sob demanda e guardar respostas acessadas em cache limitado no servidor por até 24 horas, com cache do navegador como complemento.
- Usar SQLite como arquivo local para o índice e o cache, sem exigir um servidor de banco separado.
- Na primeira execução, oferecer as votações recentes assim que estiverem disponíveis e preencher o índice desde 2023 em segundo plano, exibindo o progresso da cobertura histórica.
- Atualizar dados recentes diariamente, revisar os históricos periodicamente e recuperar atualizações pendentes quando a aplicação voltar a funcionar.
- Tratar uma resposta em cache como atual por até 24 horas. Se a fonte falhar depois disso, mostrar uma cópia antiga ainda guardada com sua data; se não houver cópia, indicar indisponibilidade.
- Permitir execução local com `cargo run` e oferecer também uma opção em contêiner para uso futuro no homelab.

## Fontes e limites conhecidos

- A [Câmara dos Deputados](https://dadosabertos.camara.leg.br/howtouse/2020-02-07-dados-votacoes.html) publica votações de Plenário e comissões, mas sua lista de votos individuais não inclui deputados ausentes. Também alerta que algumas votações simbólicas têm registros individuais parciais.
- A [API da Câmara](https://dadosabertos.camara.leg.br/swagger/api.html) oferece histórico de deputados e presenças registradas em eventos. Presença no evento não comprova presença no instante de cada votação, e ausência da lista de presenças não basta para afirmar falta.
- O [Senado Federal](https://legis.senado.leg.br/dadosabertos/v3/api-docs) publica votos nominais individuais por API. Votações secretas trazem totais, sem votos individuais públicos.
- O [guia oficial de assiduidade do Senado](https://www12.senado.leg.br/assessoria-de-imprensa/guia-para-jornalistas/tutorial-de-verificacao-da-assiduidade-dos-senadores) orienta consultar os Diários do Senado e justificativas para verificar faltas; a API de votações, por si só, não confirma ausência individual.
- A [Câmara explica](https://www2.camara.leg.br/a-camara/documentos-e-pesquisa/arquivo/acervo-por-tipo/sites-tematicos/simplificou/perguntas-frequentes-1) que PEC, PLP, PL e PDL de tramitação bicameral passaram a usar sigla, número e ano unificados a partir de 2019; propostas anteriores podem ter recebido uma nova identificação ao chegar à outra casa.
- A [API do Senado](https://legis.senado.leg.br/dadosabertos/v3/api-docs) pode responder com HTTP 429 acima de 10 requisições por segundo. As [listagens da Câmara](https://dadosabertos.camara.leg.br/swagger/api.html) são paginadas, com 15 itens por padrão e limite de 100 por requisição. Esses limites importam para consultas a cada acesso.
- A [API da Câmara](https://dadosabertos.camara.leg.br/api/v2/api-docs) não oferece filtro de votações por parlamentar nem por texto. A [API do Senado](https://legis.senado.leg.br/dadosabertos/v3/api-docs) oferece filtro de votações por parlamentar, mas não busca textual diretamente nas votações. Isso limita uma busca histórica uniforme sem um índice local.

## Verificações para implementação

- Validar com amostras reais a reconstrução da composição e da filiação em 2023 ou depois, inclusive suplentes e afastamentos.
- Verificar quais ausências podem ser confirmadas automaticamente em cada casa sem inferir falta a partir de voto ou presença não registrados.
- Definir intervalo operacional de revisão do histórico e limite de tamanho do cache, mantendo as regras de atualização e transparência acordadas acima.
