# MCP TOOLS Monorepo

MCP TOOLS — это общий монорепозиторий для всех наших решений на Model Context Protocol (MCP). Первая поставка — **MCP MultiTool**, stdio‑сервер/клиент на Rust, который помогает агентам быстро инспектировать и стресс‑тестировать сторонние MCP‑совместимые сервисы.

## Что внутри

| Путь | Назначение |
| --- | --- |
| `tools/mcp-multi-tool/` | Бинарный crate MCP MultiTool (stdio MCP сервер + инспектор целевых MCP). |
| `data/` | Лёгкие артефакты (например, sample outbox/dlq). |
| `docs/` | Архитектура, публичные контракты, схемы (заполняется по мере готовности). |
| `config/` | Конфигурация по принципу Contract‑First (будет добавляться вместе с новыми сервисами). |

## Быстрый старт

```bash
git clone git@github.com:iMAGRAY/MCP-TOOLS.git
cd MCP-TOOLS
cargo run -p mcp_multi_tool
```

Агент, совместимый с MCP (Codex CLI, Claude Code, Gemini Code Assist и др.), может подключаться к бинарю через stdio без дополнительных флагов.

## Профиль качества

- Архитектура: Modular Monolith с DDD, Ports & Adapters, CQRS по необходимости.
- Контракты: MCP 2025-06-18, rmcp `=0.8.1`, JSON Schema для событий.
- Надёжность: идемпотентность `idempotency_key`, transactional outbox, reaper TTL 60s.
- Метрики: Prometheus `/metrics`, защищённый доступ (TLS+Auth, dev-флаг `ALLOW_INSECURE_METRICS_DEV=true`).
- Тесты: `cargo test` + property/race/coverage ≥85% (Statements/Lines) — см. `CONTRIBUTING.md`.
- CI: GitHub Actions `ci.yml` (fmt, clippy, tests, покрытие).

## MCP MultiTool — фичи MVP

- Быстрое подключение к целевому MCP (stdio/SSE/HTTP) и инспекция `list_tools`, `describe`, `call`, `stream`.
- Смок‑сценарий `cargo run -p mcp_multi_tool --bin smoketest` для самопроверки.
- Интеграционный тест `tests/interop.rs`, который поднимает сервер и исполняет базовые запросы.

## Разработка

```bash
# Форматирование и быстрые проверки
cargo fmt
cargo clippy --all-targets --all-features

# Юнит и интеграция
cargo test

# Покрытие (требуется llvm-tools-preview)
cargo llvm-cov --lcov --output-path coverage.lcov --fail-under-lines 85
```

Подробные правила и чек-листы см. в `CONTRIBUTING.md`. Новые инструменты добавляй в `tools/<tool-name>` и регистрируй в workspace.

## Лицензия

Проект распространяется по лицензии [The Unlicense](LICENSE) — можно делать что угодно, без ограничений.
