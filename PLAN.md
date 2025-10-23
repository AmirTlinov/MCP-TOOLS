# Plan.md — MCP Stdio Server + MCP Inspector (Rust, rmcp 0.8.1)

## 1) Executive Summary
- Цель: единый Rust-бинарник MCP stdio сервера с инструментом mcp_inspector для когнитивно лёгкого тестирования любых MCP‑серверов (stdio/SSE/HTTP).
- Архитектура: Modular Monolith (DDD, Ports & Adapters), публичный MCP-интерфейс без флагов; конфиг только через `config/` и ENV.
- MVP: инспекция целевого MCP — подключение, список инструментов, описание, вызовы, стриминг, комплаенс‑сьют, метрики, отчёт.
- Качество: покрытие ≥85% (Statements/Lines), SLO p99 ≤200мс, ExactlyOnce ratio 1.00±0.01, без циклов зависимостей.
- Надёжность: идемпотентность через `idempotency_key`, SingleEffect (CLAIM|OUTBOX), reaper stuck TTL=60с.
- Поставки: M-0001 Kickoff → M-0002 MVP Alpha (2 недели) → M-0003 Beta (3,5 недели) → M-0004 GA (5 недель).
- Результат: бинарник, публичные контракты и JSON Schema событий, инструкции для Codex/Claude/Gemini.

## 2) Цели и метрики успеха
- G1: Доставить единый бинарник без CLI‑флагов к M-0004; Метрика: релизный артефакт для Linux/macOS/Windows; Проверка: скачивание и запуск.
- G2: Инспектор покрывает 95% типовых MCP‑операций (connect/list/describe/call/stream); Метрика: прохождение комплаенс‑сьюта ≥95%; Проверка: сценарные E2E‑тесты.
- G3: Качество кода; Метрика: Coverage Statements/Lines ≥85%; Проверка: CI gate `CI_FAIL_ON_COVERAGE_BREACH=true`.
- G4: Производительность; Метрика: p99 из `gateway_calls/logical_charges` ≤200мс на 1k RPS локально; Проверка: нагрузочные тесты.
- G5: Надёжность; Метрика: ExactlyOnce ratio 1.00±0.01 (окно 15м); Проверка: метрика стабильна в 3 прогонах.
- G6: Наблюдаемость; Метрика: `/metrics` (Prometheus) документирован, защищён (Auth+TLS), dev‑флаг для локалки; Проверка: скрейп и валидация полей.
- G7: Архитектура; Метрика: 0 циклов, соблюдение слоёв, лимиты файлов/классов/функций; Проверка: архитектурные тесты.

## 3) Опции реализации и trade-offs
- Вариант A: Чистый Modular Monolith (рекомендация)
  - + Простота деплоя, 1 бинарь, быстрые итерации, строгие слои.
  - − Меньше изоляции модулей, но компенсируется архитектурными тестами.
- Вариант B: Многокрейтный workspace (домены отдельно)
  - + Сильнее изоляция, параллельная сборка.
  - − Сложнее релиз/версионирование, потенциальная фрагментация.
- Вариант C: Плагинная система для инспектора
  - + Расширяемость.
  - − Дольше MVP, усложнение API.
- Рекомендация: Вариант A сейчас; Вариант B/C — после GA при необходимости.

## 4) Scope / Non-Goals
- Scope:
  - MCP stdio сервер (rmcp 0.8.1) с inspector‑tools.
  - Клиенты к целевым MCP: stdio, SSE, HTTP streamable.
  - Комплаенс‑сьют v0.9, отчёты, Prometheus‑метрики, JSON Schema для событий.
- Non-Goals:
  - GUI/веб‑панель.
  - Долгоживущая БД (используем in‑memory/файловый outbox).
  - Полная авторизация к целевым серверам (минимум: токены/CA‑траст).

## 5) WBS (WS→Эпики→Фичи→T-задачи)
- WS-001 Архитектура и каркас
  - Эпик: DDD/Слои/Контракты → T-0001..T-0006, T-0036..T-0038
- WS-002 Транспорты клиента (целевой MCP)
  - Эпик: stdio/SSE/HTTP → T-0007..T-0010
- WS-003 Inspector функционал
  - Эпик: базовые операции → T-0011..T-0017, T-0030
- WS-004 Надёжность/Side-effects
  - Эпик: Idempotency/Outbox/Reaper → T-0014, T-0031..T-0033
- WS-005 Наблюдаемость и SLO
  - Эпик: Метрики/Логи/Документация → T-0018..T-0019, T-0035
- WS-006 Контракты/Схемы/Документация
  - Эпик: JSON Schema/Публичные контракты → T-0020..T-0021, T-0027
- WS-007 Тестирование и CI
  - Эпик: Unit/Property/Race/E2E/CI → T-0022..T-0026, T-0038
- WS-008 Безопасность и выпуск
  - Эпик: TLS/Secret mgmt/Release → T-0028..T-0029, T-0034

### WBS_Table
ID | ParentID | Name | Level | Owner
---|---|---|---|---
WS-001 | - | Архитектура и каркас | WS | AR
WS-002 | - | Транспорты клиента | WS | RL
WS-003 | - | Inspector функционал | WS | RL
WS-004 | - | Надёжность/Side-effects | WS | AR
WS-005 | - | Наблюдаемость и SLO | WS | SRE
WS-006 | - | Контракты/Схемы/Документация | WS | DOC
WS-007 | - | Тестирование и CI | WS | QA
WS-008 | - | Безопасность и выпуск | WS | SEC

## 6) Атомарные задачи (спецификация)
id: T-0001  
title: Создать каркас проекта (workspace + crate)  
outcome: Репозиторий с каркасом, сборка проходит  
inputs: [AGENTS.md, требования]  
outputs: [workspace `Cargo.toml`, crate `mcp_inspector`]  
procedure:
  - Инициализировать workspace и бинарный crate.
  - Подключить базовые зависимости (`anyhow`, `tracing`).  
acceptance_criteria:
  - CI сборка `cargo build` успешна на 3 ОС.  
owner: RL  
effort: 4h  
dependencies: []  
risks_local:
  - id: RISK-001
    failure_mode: Неверная структура workspace
    impact: M
    likelihood: M
    mitigation: Использовать шаблон cargo; ревью AR
    contingency: Рефактор структуры до M-0002
notes: Стартовая точка всех задач

id: T-0002  
title: Зафиксировать rmcp 0.8.1 и версии зависимостей  
outcome: Репитабельная сборка с lockfile  
inputs: [Cargo.toml]  
outputs: [Cargo.toml, Cargo.lock]  
procedure:
  - Пиновать rmcp=0.8.1 и версии транспортах.
  - Зафиксировать фичи транспорта.  
acceptance_criteria:
  - `cargo tree` стабилен; `cargo deny` без критических.  
owner: RL  
effort: 2h  
dependencies: [T-0001]  
risks_local:
  - id: RISK-002
    failure_mode: Конфликт фич rmcp
    impact: M
    likelihood: M
    mitigation: Минимальный набор фич
    contingency: Патч версии/флагов

id: T-0003  
title: Смоделировать домен InspectionRun (STATE_MACHINE)  
outcome: Структуры/инварианты состояний pending→processing→captured|failed  
inputs: [AGENTS.md]  
outputs: [модуль domain, тесты инвариантов]  
procedure:
  - Определить сущности: InspectionRun, Step, Claim.
  - Задать переходы и проверки.  
acceptance_criteria:
  - Тесты на запрет skip‑states проходят.  
owner: AR  
effort: 6h  
dependencies: [T-0001]  
risks_local:
  - id: RISK-003
    failure_mode: Утечка доменной логики вверх
    impact: M
    likelihood: M
    mitigation: Порты для переходов
    contingency: Архитектурный рефактор

id: T-0004  
title: Реализовать конфиг через ENV/`config/`  
outcome: Чтение настроек без CLI флагов  
inputs: [ENV, config/*]  
outputs: [модуль config, docs]  
procedure:
  - Определить схему конфига.
  - Реализовать чтение с приоритетом ENV.  
acceptance_criteria:
  - Переменные влияют на поведение; тесты проходят.  
owner: AR  
effort: 4h  
dependencies: [T-0001]  
risks_local:
  - id: RISK-004
    failure_mode: Доступ к ENV вне config
    impact: L
    likelihood: M
    mitigation: Линт/архитектурные тесты
    contingency: Быстрый фикс с ревью

id: T-0005  
title: Скелет stdio MCP сервера (Ports)  
outcome: Сервер стартует, отвечает handshake  
inputs: [rmcp 0.8.1]  
outputs: [модуль adapters:server_stdio]  
procedure:
  - Поднять stdio‑сервер, зарегистрировать базовый tool.
  - Реализовать health‑probe tool.  
acceptance_criteria:
  - Интеракция handshake в тесте успешна.  
owner: RL  
effort: 6h  
dependencies: [T-0002,T-0003,T-0004]  
risks_local:
  - id: RISK-005
    failure_mode: Несоответствие MCP next spec
    impact: H
    likelihood: M
    mitigation: Сверка по дате 2025‑06‑18
    contingency: Совместимость слоем адаптера

id: T-0006  
title: Реестр tools и контракт Public API  
outcome: Декларативная регистрация inspector‑tools  
inputs: [арх.решения]  
outputs: [registry, manifest JSON]  
procedure:
  - Реализовать registry + декларативный манифест.
  - Экспорт списка tools через MCP.  
acceptance_criteria:
  - `list_tools` даёт корректный список.  
owner: RL  
effort: 4h  
dependencies: [T-0005]  
risks_local:
  - id: RISK-006
    failure_mode: Жёсткие связи tool↔сервис
    impact: M
    likelihood: M
    mitigation: Инверсия зависимостей
    contingency: Рефактор интерфейсов

id: T-0007  
title: Клиент целевого MCP: stdio  
outcome: Подключение к целевому stdio MCP  
inputs: [rmcp transports]  
outputs: [client_stdio]  
procedure:
  - Реализовать запуск и пайпинг stdio‑процесса.
  - Хэндшейк и heartbeat.  
acceptance_criteria:
  - E2E к mock‑серверу стабилен.  
owner: BE  
effort: 6h  
dependencies: [T-0002,T-0005]  
risks_local:
  - id: RISK-007
    failure_mode: Зависание процесса
    impact: M
    likelihood: M
    mitigation: Таймауты/kill
    contingency: Изоляция запусков

id: T-0008  
title: Клиент целевого MCP: SSE  
outcome: Подключение к SSE endpoint с токеном  
inputs: [URL, токен]  
outputs: [client_sse]  
procedure:
  - Реализовать подписку SSE, парсинг событий.
  - Реконнект/бэк‑офф.  
acceptance_criteria:
  - Потеря связи восстанавливается <5с.  
owner: BE  
effort: 6h  
dependencies: [T-0002,T-0004]  
risks_local:
  - id: RISK-008
    failure_mode: Потеря событий
    impact: H
    likelihood: M
    mitigation: Буфер/ACK
    contingency: Повторная синхронизация

id: T-0009  
title: Клиент целевого MCP: HTTP streamable  
outcome: Вызов инструментов с потоковыми ответами  
inputs: [URL, токен]  
outputs: [client_http_stream]  
procedure:
  - Реализовать запрос, чтение чанков/мультипарт.
  - Таймауты и отмена.  
acceptance_criteria:
  - Потоки собираются без утечек.  
owner: BE  
effort: 6h  
dependencies: [T-0002,T-0004]  
risks_local:
  - id: RISK-009
    failure_mode: Блокировка при backpressure
    impact: H
    likelihood: M
    mitigation: Асинк каналы/границы
    contingency: Ограничение скорости

id: T-0010  
title: `ping/handshake` и autodiscovery  
outcome: Проверка доступности и версии MCP  
inputs: [клиенты]  
outputs: [сервис probe]  
procedure:
  - Команда `probe.connect` и `probe.version`.
  - Нормализация ответов.  
acceptance_criteria:
  - Возвращает версию/транспорт/latency.  
owner: BE  
effort: 4h  
dependencies: [T-0007,T-0008,T-0009]  
risks_local:
  - id: RISK-010
    failure_mode: Несовместимые поля версий
    impact: L
    likelihood: M
    mitigation: Маппинг версий
    contingency: Фича‑флаги

id: T-0011  
title: `inspector.list_tools` целевого  
outcome: Стабильный список инструментов  
inputs: [подключение]  
outputs: [список + метаданные]  
procedure:
  - Вызвать list_tools.
  - Обогатить способностями.  
acceptance_criteria:
  - Список совпадает с эталоном mock.  
owner: RL  
effort: 3h  
dependencies: [T-0010]  
risks_local:
  - id: RISK-011
    failure_mode: Неконсистентные описания
    impact: M
    likelihood: M
    mitigation: Нормализация
    contingency: Фолбэк поля

id: T-0012  
title: `inspector.describe` схем и типов  
outcome: Получение schema/params/IO  
inputs: [target tool id]  
outputs: [описание + валидация]  
procedure:
  - Получить описание и JSON Schema.
  - Валидация локальным валидатором.  
acceptance_criteria:
  - 100% валидируемых схем на mock.  
owner: RL  
effort: 4h  
dependencies: [T-0011]  
risks_local:
  - id: RISK-012
    failure_mode: Неполные схемы
    impact: M
    likelihood: M
    mitigation: Доп. проверки
    contingency: Skip с предупреждением

id: T-0013  
title: `inspector.call` обычный вызов  
outcome: Выполнение инструмента с capture ответа  
inputs: [tool, args]  
outputs: [результат, трасса]  
procedure:
  - Отправить вызов, собрать ответ/стрим.
  - Сохранить трассу в отчёт.  
acceptance_criteria:
  - Совпадение с ожидаемым на mock.  
owner: RL  
effort: 6h  
dependencies: [T-0012]  
risks_local:
  - id: RISK-013
    failure_mode: Потеря части стрима
    impact: H
    likelihood: M
    mitigation: Буфер/флаги завершения
    contingency: Повтор с key

id: T-0014  
title: Идемпотентность через `idempotency_key`  
outcome: Повторы не создают дубликаты эффектов  
inputs: [ключ, вызов]  
outputs: [результат из кеша/повтора]  
procedure:
  - Вставить CLAIM и ключ на вызов.
  - Вернуть EXISTING при конфликте.  
acceptance_criteria:
  - Property‑тесты устойчивы в 3×100 итераций.  
owner: AR  
effort: 6h  
dependencies: [T-0013]  
risks_local:
  - id: RISK-014
    failure_mode: Коллизии ключей
    impact: M
    likelihood: L
    mitigation: UUIDv4 + scope
    contingency: external_ref_unique

id: T-0015  
title: Стриминг (`inspector.stream`)  
outcome: Поддержка частичных/финальных сообщений  
inputs: [stream handle]  
outputs: [собранный результат]  
procedure:
  - Поддержать onChunk/onFinal.
  - Сигнализация клиенту/агенту.  
acceptance_criteria:
  - Тест со смешанными чанками зелёный.  
owner: BE  
effort: 5h  
dependencies: [T-0013]  
risks_local:
  - id: RISK-015
    failure_mode: Утечки хэндлов
    impact: M
    likelihood: M
    mitigation: Drop/timeout
    contingency: Recreate stream

id: T-0016  
title: Комплаенс‑сьют v0.9  
outcome: Автотест 95% типовых операций  
inputs: [mock, реальные сервера]  
outputs: [отчёт JSON/Markdown]  
procedure:
  - Сценарии: connect/list/describe/call/stream.
  - Сводный отчёт и score.  
acceptance_criteria:
  - Отчёт формируется детерминированно.  
owner: QA  
effort: 8h  
dependencies: [T-0015]  
risks_local:
  - id: RISK-016
    failure_mode: Ложно‑отрицательные
    impact: M
    likelihood: M
    mitigation: Толеранс/ретраи
    contingency: Метки flaky

id: T-0017  
title: Латентность и SLO‑пробы  
outcome: p50/p95/p99 на операциях  
inputs: [инспектор вызовы]  
outputs: [метрики Prometheus]  
procedure:
  - Измерять время на каждом шаге.
  - Экспортировать метрики.  
acceptance_criteria:
  - p99 ≤200мс в локальном бенч.  
owner: SRE  
effort: 5h  
dependencies: [T-0016]  
risks_local:
  - id: RISK-017
    failure_mode: Шум метрик
    impact: L
    likelihood: M
    mitigation: Сэмплинг
    contingency: Агрегация окна

id: T-0018  
title: `/metrics` c Auth+TLS (+dev‑флаг)  
outcome: Защищённые метрики, dev расшивка  
inputs: [конфиг, TLS]  
outputs: [HTTP endpoint, docs]  
procedure:
  - Реализовать TLS/базовую auth.
  - Dev‑флаг `ALLOW_INSECURE_METRICS_DEV`.  
acceptance_criteria:
  - Скрейп успешен; без флага — запрещён.  
owner: SRE  
effort: 6h  
dependencies: [T-0017]  
risks_local:
  - id: RISK-018
    failure_mode: Утечка без auth
    impact: H
    likelihood: L
    mitigation: Тесты/скан
    contingency: Отключить endpoint

id: T-0019  
title: Трассировка и уровни логов  
outcome: Структурные логи с фильтром ENV  
inputs: [tracing]  
outputs: [формат логов, гайды]  
procedure:
  - Настроить `tracing_subscriber`.
  - Поля request_id/run_id.  
acceptance_criteria:
  - Логи воспроизводят путь вызова.  
owner: SRE  
effort: 3h  
dependencies: [T-0005]  
risks_local:
  - id: RISK-019
    failure_mode: Шум/PII
    impact: M
    likelihood: M
    mitigation: Редакция полей
    contingency: Маскировка

id: T-0020  
title: JSON Schema для событий/отчётов  
outcome: Версионированные схемы и валидация  
inputs: [события инспектора]  
outputs: [schemas/*.json]  
procedure:
  - Описать события (start/step/final/error).
  - Включить валидатор.  
acceptance_criteria:
  - Валидация 100% отчётов.  
owner: DOC  
effort: 4h  
dependencies: [T-0016]  
risks_local:
  - id: RISK-020
    failure_mode: Дрифт схем/кода
    impact: M
    likelihood: M
    mitigation: CI‑проверка
    contingency: Версионирование

id: T-0021  
title: Публичные контракты MCP (manifest)  
outcome: Публикованы и протестированы контракты  
inputs: [registry]  
outputs: [docs/public_api.md, manifest]  
procedure:
  - Сгенерировать manifest из registry.
  - Задокументировать параметры/ошибки.  
acceptance_criteria:
  - Агент может вызвать каждый tool по доке.  
owner: DOC  
effort: 4h  
dependencies: [T-0006,T-0020]  
risks_local:
  - id: RISK-021
    failure_mode: Несходимость manifest
    impact: M
    likelihood: L
    mitigation: Автоген
    contingency: Ручная правка

id: T-0022  
title: CI: build/test/clippy/coverage gates  
outcome: CI ломает сборку ниже 85%  
inputs: [pipeline]  
outputs: [workflow, badges]  
procedure:
  - Настроить матрицы ОС.
  - Включить cobertura/junit отчеты.  
acceptance_criteria:
  - Gate падает при 84.9%.  
owner: QA  
effort: 6h  
dependencies: [T-0001,T-0016]  
risks_local:
  - id: RISK-022
    failure_mode: Нестабильные тесты
    impact: M
    likelihood: M
    mitigation: Ретраи/таймауты
    contingency: Изоляция flaky

id: T-0023  
title: Mock MCP сервер для E2E  
outcome: Детерминированный стенд тестов  
inputs: [rmcp]  
outputs: [mock_server crate]  
procedure:
  - Реализовать примитивные инструменты.
  - Предсказуемые ответы/стримы.  
acceptance_criteria:
  - E2E проходит локально/CI.  
owner: QA  
effort: 6h  
dependencies: [T-0005]  
risks_local:
  - id: RISK-023
    failure_mode: Дрифт API
    impact: M
    likelihood: M
    mitigation: Контракты тестами
    contingency: Генерация фикстур

id: T-0024  
title: Property‑тесты идемпотентности  
outcome: Доказательство устойчивости CLAIM  
inputs: [генераторы]  
outputs: [proptests]  
procedure:
  - Смоделировать конкурентные повторные вызовы.
  - Проверить ExactlyOnce.  
acceptance_criteria:
  - 3× прогона ×100 сидов зелёные.  
owner: QA  
effort: 5h  
dependencies: [T-0014]  
risks_local:
  - id: RISK-024
    failure_mode: Ложная строгость
    impact: L
    likelihood: M
    mitigation: Диапазоны
    contingency: Смягчить инварианты

id: T-0025  
title: Race/Concurrency тесты (>=32 потоков)  
outcome: Отсутствие дедлоков/гонок  
inputs: [race harness]  
outputs: [stress tests]  
procedure:
  - Настроить многопоточность/повторы.
  - Заснять метрики ожиданий.  
acceptance_criteria:
  - p99 lock wait ≤50мс.  
owner: QA  
effort: 6h  
dependencies: [T-0015]  
risks_local:
  - id: RISK-025
    failure_mode: Фальш‑позитивы
    impact: L
    likelihood: M
    mitigation: Pin seed
    contingency: Увеличить повторы

id: T-0026  
title: Crash/Edge тесты  
outcome: Без крашей/паник на границах  
inputs: [fuzz/edge cases]  
outputs: [edge suite]  
procedure:
  - Составить набор предельных вводов.
  - Проверить отказоустойчивость.  
acceptance_criteria:
  - 0 паник за 3 прогона.  
owner: QA  
effort: 5h  
dependencies: [T-0013,T-0015]  
risks_local:
  - id: RISK-026
    failure_mode: Невидимые пути
    impact: M
    likelihood: M
    mitigation: Трассировка
    contingency: Расширить охват

id: T-0027  
title: Документация How‑To для Codex/Claude  
outcome: Пошаговые сценарии использования  
inputs: [публичные контракты]  
outputs: [README.md, guides/*]  
procedure:
  - Описать подключение и вызовы.
  - FAQ по ошибкам.  
acceptance_criteria:
  - Независимый инженер проходит гайд ≤15м.  
owner: DOC  
effort: 4h  
dependencies: [T-0021]  
risks_local:
  - id: RISK-027
    failure_mode: Пропуски шагов
    impact: M
    likelihood: M
    mitigation: Ревью QA
    contingency: Дополнение

id: T-0028  
title: Сборка релизных бинарей (3 ОС)  
outcome: Подписанные артефакты релиза  
inputs: [CI]  
outputs: [бинарники, checksums]  
procedure:
  - Кросс‑сборка, архивы.
  - Хэши и подписи.  
acceptance_criteria:
  - Скачивание и запуск успешны.  
owner: SRE  
effort: 6h  
dependencies: [T-0022]  
risks_local:
  - id: RISK-028
    failure_mode: Несовместимые таргеты
    impact: M
    likelihood: M
    mitigation: cross-rs
    contingency: Ограничить таргеты

id: T-0029  
title: Безопасность: TLS/секреты/скан зависимостей  
outcome: Базовая модель угроз закрыта  
inputs: [конфиг, deps]  
outputs: [policy.md, сканы]  
procedure:
  - TLS для `/metrics`, секреты из ENV.
  - `cargo audit` и фиксы.  
acceptance_criteria:
  - 0 крит‑уязвимостей.  
owner: SEC  
effort: 5h  
dependencies: [T-0018]  
risks_local:
  - id: RISK-029
    failure_mode: Утечки секретов
    impact: H
    likelihood: L
    mitigation: Не логировать секреты
    contingency: Ротация

id: T-0030  
title: E2E с реальным сервером (пример rmcp)  
outcome: Кросс‑проверка вне mock  
inputs: [публичный MCP пример]  
outputs: [отчёт совместимости]  
procedure:
  - Прогнать комплаенс против примера.
  - Задокументировать отличия.  
acceptance_criteria:
  - Прохождение ≥90% тестов.  
owner: QA  
effort: 6h  
dependencies: [T-0016,T-0023]  
risks_local:
  - id: RISK-030
    failure_mode: Недоступность стенда
    impact: M
    likelihood: M
    mitigation: Локальная копия
    contingency: Отложить на Beta

id: T-0031  
title: Transactional Outbox (файловый/in‑mem)  
outcome: Гарантия доставки событий  
inputs: [события]  
outputs: [outbox + ретраер]  
procedure:
  - Писать в outbox атомарно.
  - Ретраи/экспоненты.  
acceptance_criteria:
  - 0 потерь при сбоях в тестах.  
owner: AR  
effort: 6h  
dependencies: [T-0013]  
risks_local:
  - id: RISK-031
    failure_mode: Потеря при падении
    impact: M
    likelihood: L
    mitigation: fsync
    contingency: Повтор записи

id: T-0032  
title: Reaper stuck TTL=60с  
outcome: Авто‑разблокировка зависших processing  
inputs: [run state]  
outputs: [reaper job]  
procedure:
  - Скан по TTL, перевод в failed.
  - Метрика и событие.  
acceptance_criteria:
  - Тест с зависанием — успешный reaper.  
owner: AR  
effort: 3h  
dependencies: [T-0031]  
risks_local:
  - id: RISK-032
    failure_mode: Ложные срабатывания
    impact: L
    likelihood: M
    mitigation: Буфер TTL
    contingency: Повторная проверка

id: T-0033  
title: Compensation для external_ref_unique  
outcome: Восстановление консистентности при коллизиях  
inputs: [конфликты]  
outputs: [компенсации]  
procedure:
  - Определить шаги отката/повтора.
  - Логика выбора пути.  
acceptance_criteria:
  - Компесации проходят тесты сценариев.  
owner: AR  
effort: 5h  
dependencies: [T-0031,T-0014]  
risks_local:
  - id: RISK-033
    failure_mode: Двойные эффекты
    impact: H
    likelihood: L
    mitigation: Проба на dry‑run
    contingency: Manual fence

id: T-0034  
title: Canary флаг и Rollback Fast  
outcome: Управление включением inspector‑фич  
inputs: [config]  
outputs: [feature flags]  
procedure:
  - Ввести флаги изменения поведения.
  - Документация rollback.  
acceptance_criteria:
  - Фича переключается без перезапуска.  
owner: SRE  
effort: 3h  
dependencies: [T-0006,T-0019]  
risks_local:
  - id: RISK-034
    failure_mode: Дрифт конфигов
    impact: L
    likelihood: M
    mitigation: Валидация
    contingency: Значения по умолчанию

id: T-0035  
title: ErrorBudget Freeze политика  
outcome: Чек‑лист заморозки релиза  
inputs: [SLO метрики]  
outputs: [policy doc, CI gate]  
procedure:
  - Определить пороги/окна.
  - Включить gate при нарушении.  
acceptance_criteria:
  - Gate блокирует релиз при нарушении.  
owner: PM  
effort: 4h  
dependencies: [T-0017,T-0022]  
risks_local:
  - id: RISK-035
    failure_mode: Ложные блокировки
    impact: M
    likelihood: M
    mitigation: Буферы
    contingency: Override с approval

id: T-0036  
title: ACL границы и анти‑зависимости  
outcome: Слои соответствуют правилам Deps_Allow  
inputs: [арх. требования]  
outputs: [арх‑тесты, build‑rule]  
procedure:
  - Реализовать проверку импортов.
  - Запрет относительных родительских импортов.  
acceptance_criteria:
  - Нарушение падает в CI.  
owner: AR  
effort: 4h  
dependencies: [T-0001]  
risks_local:
  - id: RISK-036
    failure_mode: Ложные флаги
    impact: L
    likelihood: M
    mitigation: Исключения
    contingency: Уточнение правил

id: T-0037  
title: Стат‑анализ и линтинги  
outcome: clippy/deny/cargo‑audit чистые  
inputs: [код]  
outputs: [конфиги, отчёты]  
procedure:
  - Настроить правила clippy/deny.
  - Включить в CI.  
acceptance_criteria:
  - 0 крит‑ошибок в отчётах.  
owner: QA  
effort: 4h  
dependencies: [T-0022]  
risks_local:
  - id: RISK-037
    failure_mode: Слишком жёсткие правила
    impact: L
    likelihood: M
    mitigation: Категории
    contingency: Смягчить rule‑set

id: T-0038  
title: Архитектурные тесты (Cycles_Forbidden)  
outcome: 0 циклов, соблюдение лимитов  
inputs: [кодовая база]  
outputs: [arch tests]  
procedure:
  - Проверка графа зависимостей.
  - Лимиты файлов/функций.  
acceptance_criteria:
  - Тест падает при цикле/превышении.  
owner: QA  
effort: 5h  
dependencies: [T-0036]  
risks_local:
  - id: RISK-038
    failure_mode: Неполный граф
    impact: M
    likelihood: L
    mitigation: Парсинг cargo‑metadata
    contingency: Ручная проверка

## 7) Зависимости и критический путь (CPM)
Текстовая диаграмма:  
T-0001 → T-0002 → T-0005 → T-0006 → T-0011 → T-0012 → T-0013 → T-0015 → T-0016 → T-0017 → T-0018 → T-0022 → T-0028 → GA

Подветви:
- Клиенты: T-0007/T-0008/T-0009 → T-0010 → T-0011
- Надёжность: T-0014 → T-0031 → T-0032 → T-0033
- Тесты: T-0023 → T-0030; плюс T-0024/T-0025/T-0026
- Арх‑качество: T-0036 → T-0038

Критический путь: основная цепочка выше (жирная), буферы: 15% на интеграции, 10% на CI.

### Dependencies_Table
From | To | Type | Note
---|---|---|---
T-0001 | T-0002 | FS | init→deps
T-0002 | T-0005 | Tech | rmcp готов
T-0005 | T-0006 | Logic | реестр tools
T-0006 | T-0011 | Logic | list_tools через реестр
T-0011 | T-0012 | Data | описания зависят от списка
T-0012 | T-0013 | Flow | вызовы после описаний
T-0013 | T-0015 | Flow | стрим после вызовов
T-0015 | T-0016 | QA | комплаенс после стрима
T-0016 | T-0017 | Obs | метрики после сценариев
T-0017 | T-0018 | Sec | защитить метрики
T-0018 | T-0022 | CI | метрики в CI
T-0022 | T-0028 | Release | артефакты
T-0007 | T-0010 | Flow | stdio клиент→probe
T-0008 | T-0010 | Flow | sse клиент→probe
T-0009 | T-0010 | Flow | http клиент→probe
T-0014 | T-0031 | Logic | idem→outbox
T-0031 | T-0032 | Ops | reaper
T-0032 | T-0033 | Ops | компенсации

## 8) График и вехи
MID | Name | Entry_Criteria | Exit_Criteria | Target_Date
---|---|---|---|---
M-0001 | Kickoff | Подтверждён план | Создан каркас и deps | 2025-10-23
M-0002 | MVP Alpha | Завершены T-0001..T-0013 | Интерактивная инспекция базовых операций | 2025-11-04
M-0003 | Beta | +T-0014..T-0019,T-0023 | Комплаенс‑сьют ≥90%, метрики, логи | 2025-11-14
M-0004 | GA 1.0 | +T-0020..T-0029,T-0031..T-0038 | Покрытие ≥85%, p99≤200мс, релизы | 2025-11-25
M-0005 | Post‑GA Hardening | Feedback приняты | Устранены TOP‑риски | 2025-12-05

## 9) Риски (FMEA) и меры
RID | Failure_Mode | Impact(H/M/L) | Likelihood(H/M/L) | Mitigation | Owner
---|---|---|---|---|---
RISK-001 | Ошибка структуры workspace | M | M | Шаблон cargo, ревью | AR
RISK-005 | Несоответствие MCP spec 2025‑06‑18 | H | M | Сверка и адаптер | RL
RISK-007 | Хэнг stdio при запуске target | M | M | Таймауты/kill | BE
RISK-008 | Потеря SSE событий | H | M | Буфер/ACK/реконнект | BE
RISK-009 | Backpressure в HTTP потоках | H | M | Асинк‑каналы/лимиты | BE
RISK-013 | Потеря частей стрима | H | M | Финальные флаги/ретраи | RL
RISK-018 | Метрики без auth/TLS | H | L | Защита + тесты | SRE
RISK-021 | Дрифт manifest↔код | M | L | Автоген/CI проверка | DOC
RISK-022 | Flaky тесты CI | M | M | Ретраи/изоляция | QA
RISK-029 | Утечки секретов | H | L | Masking/ENV only | SEC

Mitigation и contingency включены локально в задачах.

## 10) RACI
Work_Item | R | A | C | I
---|---|---|---|---
Архитектура/DDD | AR | PM | RL, QA | SEC, SRE, DOC
Stdio сервер/реестр | RL | AR | BE | QA
Клиенты stdio/SSE/HTTP | BE | RL | AR | QA
Inspector API/Compliance | RL | PM | QA | DOC
Надёжность/Outbox/Reaper | AR | PM | SRE | RL
Метрики/Логи | SRE | AR | QA | PM
CI/Тесты | QA | PM | RL | AR
Документация/Контракты | DOC | PM | RL | QA
Безопасность/TLS | SEC | PM | SRE | AR
Релизы | SRE | PM | SEC | Все

## 11) Качество и тест-план
- Уровни:
  - Unit (домены/порты) — покрытие ≥85%.
  - Property (идемпотентность, ExactlyOnce).
  - Race/Stress (≥32 потока, 3 повтора, seed pinned).
  - Edge/Crash — 0 паник.
  - E2E (mock + реальный пример rmcp).
- Фазы:
  - Alpha: Unit/E2E mock, базовые метрики.
  - Beta: Property/Race/Edge, комплаенс ≥90%.
  - GA: Все гейты, coverage ≥85%, p99≤200мс.
- Критерии входа/выхода:
  - Вход: CI зелёный на предыдущем уровне.
  - Выход: Метрики и отчёты в артефактах CI.

## 12) Runbooks/SOP
- Запуск сервера:
  - Установи ENV/`config/*` (без флагов), запусти бинарь.
  - Проверь `/metrics` (dev‑флаг в локали).
- Подключение к целевому MCP:
  - Выбери транспорт (stdio/SSE/HTTP) в конфиге.
  - Выполни `inspector.probe` → `list_tools`.
- Комплаенс‑сьют:
  - Запусти `inspector.compliance` с target‑параметрами.
  - Получи отчёт JSON/Markdown.
- Разбор зависаний:
  - Проверь reaper логи, события `stuck.reaped`.
  - Перезапусти run, валидация идемпотентности.
- Роллбек:
  - Отключи фичу канареечного флага.
  - Верни стабильную конфигурацию.

## 13) Коммуникации и Change Control
- Ритм: Daily standup (15м), Weekly review (30м), Milestone demo.
- Формат статусов: Red/Amber/Green + КПIs (coverage, p99, pass%).
- Change Control: RFC (≤1 стр), оценка влияния, решение AR/PM, версия плана (semver планов).

## 14) Оценки бюджета/усилий
- Оценка команды: 1 AR, 1 RL/BE, 1 BE, 1 QA, 1 SRE, 0.5 SEC, 0.5 DOC, 0.5 PM.
- Диапазоны (опт/реал/песс), суммарно на 5 недель спринта.

### Budget_Table
Item | Qty | Unit_Cost | Total | Notes
---|---|---|---|---
Dev hrs (AR/RL/BE) | 3×160h | $80/h | $38,400 | 5 недель
QA hrs | 160h | $60/h | $9,600 | тест‑план+E2E
SRE hrs | 120h | $70/h | $8,400 | CI/релизы/метрики
SEC hrs | 40h | $90/h | $3,600 | аудит
DOC hrs | 40h | $50/h | $2,000 | гайды/контракты
Infra (CI minutes) | 5k | $0.02 | $100 | кеш cargo
Сумма | - | - | $62,100 | ±20%

## 15) Acceptance Criteria (DoD)
- Бинарник запускается без флагов, конфиг только ENV/`config/`.
- Inspector инструменты доступны через MCP, проходят комплаенс ≥95%.
- Coverage ≥85% (Statements/Lines), CI гейт активен.
- p99 ≤200мс на эталонных сценариях; ExactlyOnce 1.00±0.01.
- `/metrics` защищён (Auth+TLS), dev‑флаг работает.
- JSON Schema событий полная, валидация 100%.
- Нет циклов зависимостей; лимиты размеров соблюдены.
- Релизы для 3 ОС, скачиваются и запускаются.

## 16) Допущения и открытые вопросы
### Блокеры
- Нет: критичных блокеров не выявлено.

### Допущения
AID | Statement | Risk_if_False | Owner
---|---|---|---
A-001 | rmcp 0.8.1 совместим с MCP 2025‑06‑18 | Рефактор адаптеров | RL
A-002 | Целевые MCP поддерживают list/describe/call/stream | Недобор комплаенса | QA
A-003 | Разрешён HTTP endpoint `/metrics` | Ограничение наблюдаемости | SRE
A-004 | Codex/Claude принимают stdio MCP без флагов | Потребуются shim‑скрипты | PM

### Открытые вопросы
- Нужен ли persistent outbox для GA? (пока file/in‑mem)
- Нужен ли экспорт отчётов в JUnit формат?

## 17) Next 3 Actions (сегодня)
1. Утвердить план и IDs, зафиксировать версии зависимостей.  
2. Поднять каркас проекта и stdio‑сервер скелет.  
3. Реализовать `inspector.probe` + mock‑server для E2E.

---

## Backlog_Table
TID | Title | WS | Effort | Owner | DoD_Short | Status
---|---|---|---|---|---|---
T-0001 | Каркас проекта | WS-001 | 4h | RL | Build ok 3 ОС | Planned
T-0002 | rmcp=0.8.1 версии | WS-001 | 2h | RL | `cargo tree` стабилен | Planned
T-0003 | Домен STATE_MACHINE | WS-001 | 6h | AR | Тесты инвариантов | Planned
T-0004 | Конфиг ENV/config | WS-001 | 4h | AR | Тесты конфига | Planned
T-0005 | Старт stdio сервера | WS-001 | 6h | RL | Handshake ok | Planned
T-0006 | Реестр tools | WS-001 | 4h | RL | list_tools ok | Planned
T-0007 | Клиент stdio | WS-002 | 6h | BE | E2E mock ok | Planned
T-0008 | Клиент SSE | WS-002 | 6h | BE | Реконнект <5с | Planned
T-0009 | Клиент HTTP stream | WS-002 | 6h | BE | Потоки ок | Planned
T-0010 | Probe/handshake | WS-002 | 4h | BE | Версия+latency | Planned
T-0011 | list_tools target | WS-003 | 3h | RL | Список совпал | Planned
T-0012 | describe schemas | WS-003 | 4h | RL | Валидация ок | Planned
T-0013 | call инструмент | WS-003 | 6h | RL | Результат/трасса | Planned
T-0014 | Идемпотентность | WS-004 | 6h | AR | Proptests ok | Planned
T-0015 | Стриминг | WS-003 | 5h | BE | Чанки ок | Planned
T-0016 | Комплаенс‑сьют | WS-003 | 8h | QA | Отчёт детерм. | Planned
T-0017 | SLO‑пробы | WS-005 | 5h | SRE | p99 ≤200мс | Planned
T-0018 | /metrics TLS/Auth | WS-005 | 6h | SRE | Защищён | Planned
T-0019 | Логи/трасс | WS-005 | 3h | SRE | Поля id | Planned
T-0020 | JSON Schema событий | WS-006 | 4h | DOC | Валидация 100% | Planned
T-0021 | Публичные контракты | WS-006 | 4h | DOC | Гайд вызовов | Planned
T-0022 | CI gates | WS-007 | 6h | QA | Gate 85% | Planned
T-0023 | Mock MCP | WS-007 | 6h | QA | E2E ok | Planned
T-0024 | Proptests idem | WS-007 | 5h | QA | 3×100 сидов | Planned
T-0025 | Race tests | WS-007 | 6h | QA | lock p99≤50мс | Planned
T-0026 | Crash/Edge | WS-007 | 5h | QA | 0 паник | Planned
T-0027 | How‑To docs | WS-006 | 4h | DOC | Проход ≤15м | Planned
T-0028 | Релизы бинарей | WS-008 | 6h | SRE | Старт 3 ОС | Planned
T-0029 | Security baseline | WS-008 | 5h | SEC | 0 крит vuln | Planned
T-0030 | E2E real server | WS-003 | 6h | QA | ≥90% pass | Planned
T-0031 | Outbox | WS-004 | 6h | AR | 0 потерь | Planned
T-0032 | Reaper TTL | WS-004 | 3h | AR | Reaped ok | Planned
T-0033 | Compensation | WS-004 | 5h | AR | Сценарии ok | Planned
T-0034 | Canary/Rollback | WS-008 | 3h | SRE | Toggle ok | Planned
T-0035 | ErrorBudget | WS-005 | 4h | PM | Gate работает | Planned
T-0036 | ACL/Deps rules | WS-001 | 4h | AR | CI падает при наруш. | Planned
T-0037 | Линтинги/аудит | WS-007 | 4h | QA | 0 крит | Planned
T-0038 | Арх‑тесты | WS-007 | 5h | QA | 0 циклов | Planned

---

## Quality Gates (самопроверка)
- QG-01: Все задачи имеют DoD/owner/dependencies — да.
- QG-02: ID уникальны, термины едины — да.
- QG-03: Цели S.M.A.R.T и проверяемы — да.
- QG-04: ТОП‑риски имеют mitigation/contingency — да.
- QG-05: Критический путь реалистичен, буферы учтены — да.

---

## Confidence: 82%
Ограничения: возможна коррекция по MCP 2025‑06‑18 деталям; неопределённость по реальным целевым серверам для E2E.

## Change Log Seed
- v0.1: Первичная декомпозиция и CPM.
- v0.2: Уточнение комплаенс‑сьюта и метрик.
- v0.3: Добавлены security/rollback политики.

## Next 3 Actions
1) Зафиксировать план и роли (RACI), подтвердить сроки M‑вех.  
2) Старт T‑0001/0002/0005: каркас + rmcp + stdio‑скелет.  
3) Реализовать T‑0010 probe и T‑0023 mock для ранних E2E.

