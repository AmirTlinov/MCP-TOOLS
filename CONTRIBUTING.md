# Руководство для контрибьюторов

Добро пожаловать! Чтобы поддерживать *flagship* уровень качества MCP TOOLS, соблюдай правила ниже.

## Базовые принципы
- **DDD + Modular Monolith**: новые пакеты кладём в `tools/`, слойность смотри в `tools/mcp-multi-tool/src` (`domain`, `app`, `adapters`, `infra`, `shared`).
- **Config First**: новые настройки добавляй в `config/` + отражай в документации.
- **Contract First**: публичные API/ивенты оформляй JSON Schema в `docs/contracts/` и обновляй README.

## Качество кода
- `cargo fmt` — форматирование обязательно.
- `cargo clippy --all-targets --all-features -D warnings` — без предупреждений.
- `cargo test` — все тесты (юнит/интеграция).
- `cargo llvm-cov --fail-under-lines 85` — покрытие по строкам ≥85% для изменённого кода.
- При работе со state machine (статусы `pending` → `processing` → `captured` → `failed`) пиши property-тесты на недопустимые переходы.

## Git & CI
- Ветви именуем `feature/<slug>` или `fix/<slug>`.
- Каждый PR должен проходить `ci.yml` (fmt, clippy, tests, coverage).
- Коммиты — в стиле Conventional Commits (`feat:`, `fix:`, `chore:` и т.д.).

## Безопасность и надёжность
- Любые побочные эффекты через шаблон `CLAIM|OUTBOX` с `RETURNING`.
- Метрики добавляй в `infra::metrics`, документируй в `docs/metrics.md`.
- Не хардкодь секреты, используй `dotenv`/`config/`.

## Документация
- Обновляй `AGENTS.md`, `PLAN.md`, `README.md` при изменении стратегии.
- Для новых инструментов создавай `docs/<tool>/overview.md` с архитектурой и сценариями использования.

Спасибо за вклад!
