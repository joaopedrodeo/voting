# Índice local e votos sob demanda

O site precisa pesquisar votações desde 2023 por proposta e parlamentar nas duas casas, mas a API da Câmara não oferece filtro de votações por parlamentar e consultas totalmente ao vivo repetiriam chamadas às fontes oficiais. Decidimos manter um índice local compacto de propostas, votações e vínculos com parlamentares em SQLite, buscar os votos completos quando uma página for acessada e considerar essas respostas em cache atuais por até 24 horas. Se a fonte falhar, uma cópia antiga ainda guardada pode ser exibida com sua data. Essa combinação evita importar todo o conteúdo dos votos, preserva a busca histórica e reduz chamadas repetidas às APIs.

O índice exigirá atualização periódica. Uma votação ainda não consultada dependerá da disponibilidade da fonte oficial para carregar seus votos completos.
