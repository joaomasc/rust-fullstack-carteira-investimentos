# Wallet Live — Carteira de Investimentos Fullstack em Rust

Aplicação web para cadastrar ativos de investimento e acompanhar o valor da carteira de cada pessoa usuária.
Une API, banco de dados, autenticação e páginas HTML em uma única aplicação Rust.

## O que o projeto faz

- Cadastro e login de pessoas usuárias (a conta é criada no primeiro acesso), com senha em hash e sessão por **JWT em cookie**.
- Cadastro de **ativos** (nome e valor unitário) e atualização do valor unitário.
- **Carteira por usuário**: adicionar um ativo com a quantidade, editar a quantidade e remover.
- **Dashboard** com o valor total da carteira (soma de `quantidade × valor unitário`).
- **API JSON** em `/api/assets` (listar, criar e atualizar ativos; criar e atualizar exigem o cabeçalho `Authorization` de admin).

## Tecnologias

Rust (edition 2024), [Axum](https://github.com/tokio-rs/axum), [SQLx](https://github.com/launchbadge/sqlx) + PostgreSQL,
[Askama](https://github.com/askama-rs/askama) (templates HTML), `jwt-simple`, `password-auth`, Docker Compose, `insta` (testes de snapshot) e Tailwind via CDN.

## Como executar

Pré-requisitos: Rust (via `rustup`), Docker e o [`sqlx-cli`](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli).

```bash
# 0. Uma vez só: instala o sqlx-cli (apenas com suporte a PostgreSQL)
cargo install sqlx-cli --no-default-features --features native-tls,postgres

# 1. Sobe o PostgreSQL
docker compose up -d

# 2. Cria as tabelas. Precisa vir ANTES da primeira compilação (veja a nota abaixo)
sqlx migrate run

# 3. Compila e inicia a aplicação
cargo run
```

> **Por que o banco precisa estar no ar para compilar?** As macros `sqlx::query_as!` conferem cada SQL
> contra o banco do `DATABASE_URL` durante a compilação, então as tabelas `assets` e `users` já precisam existir.
> Sem o Postgres rodando ou sem as migrations aplicadas, `cargo build`, `cargo run` e `cargo test` falham com
> `error communicating with database` ou `relation "assets" does not exist`.
> A aplicação também aplica migrations pendentes ao iniciar (`sqlx::migrate!`); depois do `sqlx migrate run`
> isso não repete nada, porque o SQLx registra na tabela `_sqlx_migrations` o que já foi aplicado.

Abra <http://localhost:3000>. O `.env` já aponta para o banco do `compose.yml`:

```
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres
```

### Rotas

| Método | Rota | Descrição |
| --- | --- | --- |
| GET | `/` | Dashboard da carteira (redireciona para `/login` se não autenticado) |
| GET/POST | `/login` | Página de login / autenticação (cria a conta se não existir) |
| POST | `/logout` | Encerra a sessão |
| POST | `/assets` | Cadastra um ativo |
| POST | `/assets/{id}/update` | Atualiza o valor unitário de um ativo |
| POST | `/holdings` | Adiciona um ativo à carteira ou atualiza a quantidade |
| POST | `/holdings/{id}/delete` | Remove uma posição da carteira |
| GET/POST/PATCH | `/api/assets` | API JSON de ativos |

## Melhorias implementadas

1. **Valor total da carteira** no dashboard (`models::wallet_total`).
2. **Nova tabela `holdings`** (migration `20260401120000_create_holdings`) ligando usuário, ativo e quantidade, com `UNIQUE (user_id, asset_id)` e `CHECK (quantity > 0)`.
3. **Dashboard com formulários** para cadastrar/editar ativos e adicionar/editar/remover posições (`templates/dashboard.html`).
4. **Validações e mensagens de erro** (`src/validation.rs`): nome obrigatório, números positivos e finitos (aceita vírgula ou ponto), usuário de 3 a 32 caracteres e senha com pelo menos 6 no cadastro. Os erros aparecem na própria página, e a API JSON também valida.
5. **Login com mensagens** de erro na página, em vez de JSON cru.
6. **Migrations automáticas** ao iniciar a aplicação (`sqlx::migrate!`).
7. **Logout** e cookie com `Path=/` e `SameSite=Lax`.
8. **API devolve 409** quando o nome do ativo já existe (antes respondia 500 com a mensagem crua do banco).
9. **`compose.yml` corrigido para o Postgres 18**: o volume passa a ser montado em `/var/lib/postgresql`, porque a imagem 18+ guarda os dados em `/var/lib/postgresql/18/docker` e não inicia com um volume em `/var/lib/postgresql/data`. A variável `POSTGRES_DATABASE`, que a imagem ignora, foi trocada por `POSTGRES_DB`.
10. **Compila no Windows sem `cmake`**: o `jwt-simple` usa a feature `pure-rust` em vez do BoringSSL, que exigiria `cmake` e compilador C. O projeto só usa HS256, que funciona igual nas duas versões.
11. **Novos testes** (abaixo).

## Como testar

Com o PostgreSQL no ar e as migrations aplicadas (passos 1 e 2 acima):

```bash
cargo test
```

São 18 testes:

- **11 unitários**, que não consultam o banco ao rodar: validações (`src/validation.rs`), total da carteira (`src/models.rs`) e formatação de valores (`src/routes/frontend.rs`). Mesmo esses precisam do banco para *compilar*, por causa das macros `query_as!`.
- **7 com banco** (`#[sqlx::test]`): 4 da API de ativos (`src/routes/api.rs`, incluindo nome duplicado → 409) e 3 do repositório de posições (`src/repository.rs`: criar, atualizar, listar e remover; isolamento entre usuários; ativo inexistente). O SQLx cria um banco temporário para cada teste, então os dados do banco principal não são tocados.

Teste manual: entre em `/login` com um usuário novo, cadastre um ativo, adicione-o à carteira com uma quantidade e confira o total no topo da página.

## Pontos de atenção (próximos passos)

- O segredo do JWT (`im-so-secret`) e a chave de admin da API (`im-the-admin`) estão fixos no código, como no projeto base. Em produção devem vir de variáveis de ambiente.
- Os formulários não têm proteção contra CSRF além do `SameSite=Lax` do cookie.
- Os ativos são compartilhados entre todas as pessoas usuárias; qualquer usuário logado pode cadastrá-los e alterar o valor unitário. Para restringir, seria preciso um papel de admin.
- Valores e quantidades usam `f64` (`DOUBLE PRECISION`), como no projeto base. Para dinheiro, o ideal seria um tipo decimal (`NUMERIC` + `rust_decimal`), que evita erros de arredondamento de ponto flutuante.

## O que aprendi

- **Organizar uma aplicação Axum em camadas.** Separei rotas, repositório, autenticação, modelos e validação em módulos. Com *extractors* (`FromRequestParts`), cada handler declara o que precisa (`Repository`, `User`, `Option<User>`, `Admin`) e o Axum entrega pronto. Assim a regra de "estar logado" fica num lugar só, e não repetida em cada rota.
- **O SQLx checa o SQL na compilação, e isso tem um custo.** As macros `query_as!` conferem cada consulta contra o banco real, então um erro de digitação numa coluna aparece no `cargo build`, não em produção. Em compensação, o banco precisa estar no ar e com as migrations aplicadas antes de compilar. Descobri isso quando o build falhou com o banco desligado. Na consulta com `JOIN` da carteira usei `query_as` com `FromRow`, que é verificada só em tempo de execução.
- **O banco é a última linha de defesa.** A tabela `holdings` tem `UNIQUE (user_id, asset_id)`, `CHECK (quantity > 0)` e chaves estrangeiras. O `UNIQUE` permitiu fazer "adicionar ou atualizar" numa única consulta com `ON CONFLICT ... DO UPDATE`. Aprendi a transformar violações do banco (`is_unique_violation`, `is_foreign_key_violation`) em erros que fazem sentido para quem usa o sistema, como 409 para nome de ativo repetido em vez de um 500 genérico.
- **Um enum de erros deixa o tratamento explícito.** Com `AppError` e `IntoResponse`, cada falha vira um status HTTP definido num só lugar. Validar a entrada antes de chegar ao banco (`src/validation.rs`) e mostrar a mensagem na própria página deixou a experiência bem melhor do que devolver JSON cru.
- **Autenticação sem guardar senha.** A senha é salva só como hash (`password-auth`), e a sessão é um JWT num cookie `HttpOnly` com `SameSite=Lax`. No logout, o cookie precisa ser removido com o mesmo `Path` com que foi criado. O Askama ajuda na segurança porque escapa o HTML dos templates automaticamente.
- **Testar com banco de verdade.** O `#[sqlx::test]` cria um banco temporário para cada teste e aplica as migrations, o que me deixou testar o isolamento entre usuários (uma pessoa não consegue remover a posição de outra). O `insta` guarda snapshots das respostas da API.
- **Dependências e infraestrutura também quebram o build.** O `jwt-simple` vinha por padrão com o BoringSSL, que exige `cmake` no Windows; ativar a feature `pure-rust` resolveu. Já o Postgres 18 mudou o lugar onde a imagem Docker guarda os dados, e o volume do `compose.yml` precisou mudar junto. Ler a documentação das *features* do crate e o entrypoint da imagem foi o que resolveu os dois casos.
- **O que eu faria diferente.** Usaria um tipo decimal (`NUMERIC` + `rust_decimal`) para valores em dinheiro em vez de `f64`, e tiraria o segredo do JWT e a chave de admin do código, passando a lê-los de variáveis de ambiente.
