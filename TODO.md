# TODO.md — Детерминированный чеклист реализации (Flagship+++)

Статусы: [ ] todo · [~] wip · [x] done · [b] blocked.
Формат пункта: `<ID> — <краткое действие> — DoD`. Все пункты привязаны к T‑ID.

## 0. Подготовка окружения
- [ ] ENV: зафиксировать переменные (`ALLOW_INSECURE_METRICS_DEV`, токены) — DoD: `.env.example` создан.
- [ ] Toolchain: `rustup 1.81+`, `cargo-deny`, `cargo-audit`, `just` — DoD: `just --list` работает.

## 1. Kickoff (M-0001)
- [ ] T-0001 — Инициализация workspace и крейта — DoD: `cargo build` ok на локали.
- [ ] T-0002 — Пин rmcp=0.8.1 и фич — DoD: `cargo tree` стабилен; `Cargo.lock` создан.
- [ ] T-0003 — Домен `InspectionRun` (STATE_MACHINE) — DoD: unit‑тесты переходов зелёные.
- [ ] T-0004 — Конфиг ENV/`config/` — DoD: unit‑тесты чтения и приоритета ENV.
- [ ] T-0005 — Stdio MCP сервер: handshake — DoD: интеграционный тест handshake зелёный.
- [ ] T-0006 — Реестр tools и manifest — DoD: `list_tools` возвращает зарегистрированные инструменты.

## 2. Клиенты целевого MCP (WS-002)
- [ ] T-0007 — Клиент stdio (spawn + pipes) — DoD: e2e к mock ок.
- [ ] T-0008 — Клиент SSE (подписка/реконнект) — DoD: восстановление <5с.
- [ ] T-0009 — Клиент HTTP streamable — DoD: корректный сбор чанков без утечек.
- [ ] T-0010 — Probe (connect/version/latency) — DoD: возвращает версию/latency/transport.

## 3. Inspector базовые операции (WS-003)
- [ ] T-0011 — `inspector.list_tools` — DoD: список совпадает с mock‑эталоном.
- [ ] T-0012 — `inspector.describe` (+JSON Schema) — DoD: 100% валидируемых описаний.
- [ ] T-0013 — `inspector.call` (с трассой) — DoD: е2е совпадает с эталоном.
- [ ] T-0015 — Стриминг onChunk/onFinal — DoD: смешанный тест зелёный.
- [ ] T-0016 — Комплаенс‑сьют — DoD: отчёт JSON/MD детерминирован.
- [ ] T-0030 — E2E c реальным примером — DoD: ≥90% pass, отчёт приложен.

## 4. Надёжность/идемпотентность (WS-004)
- [ ] T-0014 — Idempotency (CLAIM + key) — DoD: proptests устойчивы 3×100.
- [ ] T-0031 — Transactional Outbox — DoD: 0 потерь в тестах падений.
- [ ] T-0032 — Reaper TTL=60s — DoD: stuck→failed, событие/метрика сгенерированы.
- [ ] T-0033 — Compensation external_ref_unique — DoD: сценарии компенсаций зелёные.

## 5. Наблюдаемость и SLO (WS-005)
- [ ] T-0017 — Метрики latency p50/p95/p99 — DoD: Prometheus собирает метрики.
- [ ] T-0018 — `/metrics` с Auth+TLS (+dev‑флаг) — DoD: без auth запрещено; dev‑флаг работает.
- [ ] T-0019 — Трассировка и уровни логов — DoD: поля `request_id/run_id` в логах.
- [ ] T-0035 — ErrorBudget Freeze — DoD: CI gate при нарушении SLO.

## 6. Контракты/Документация (WS-006)
- [ ] T-0020 — JSON Schema событий — DoD: 100% отчётов валидны.
- [ ] T-0021 — Публичные контракты/manifest — DoD: гайд выполняется за ≤15 минут.
- [ ] T-0027 — How‑To Codex/Claude — DoD: независимый прогон успешно завершён.

## 7. Тестирование и CI (WS-007)
- [ ] T-0023 — Mock MCP для E2E — DoD: детерминированные ответы/стримы.
- [ ] T-0022 — CI: build/test/clippy/coverage — DoD: gate 85% активен.
- [ ] T-0024 — Proptests идемпотентности — DoD: 3× прогона ×100 сидов.
- [ ] T-0025 — Race/Concurrency ≥32 потоков — DoD: lock wait p99≤50мс.
- [ ] T-0026 — Crash/Edge — DoD: 0 паник.
- [ ] T-0037 — Линтинги/аудит — DoD: 0 крит уязвимостей/нарушений.
- [ ] T-0038 — Арх‑тесты deps/лимиты — DoD: 0 циклов, лимиты соблюдены.

## 8. Безопасность и релиз (WS-008)
- [ ] T-0028 — Релизы (3 ОС) — DoD: бинарники скачиваются и стартуют.
- [ ] T-0029 — Security baseline — DoD: секреты не в логах; audit чист.
- [ ] T-0034 — Canary/Rollback — DoD: фича переключается без перезапуска.

---

## Пошаговая реализация по критическому пути
1) [ ] T-0001 → 2) [ ] T-0002 → 3) [ ] T-0005 → 4) [ ] T-0006 → 5) [ ] T-0011 → 6) [ ] T-0012 → 7) [ ] T-0013 → 8) [ ] T-0015 → 9) [ ] T-0016 → 10) [ ] T-0017 → 11) [ ] T-0018 → 12) [ ] T-0022 → 13) [ ] T-0028

Буфер интеграций 15%, CI 10%.

---

## Контроль качества и гейты
- [ ] Coverage Statements/Lines ≥85% — gate в CI.
- [ ] p99 ≤200мс на эталонных сценариях.
- [ ] ExactlyOnce 1.00±0.01 (15м окно).
- [ ] `/metrics` защищён (Auth+TLS), dev‑флаг документирован.
- [ ] 0 циклов зависимостей; соблюдены лимиты (файлы/функции).

---

## Быстрые команды (подсказки)
- `just bootstrap` — установка инструментов.
- `just test-all` — unit+e2e+property+race.
- `just run-stdio` — запуск stdio сервера.
- `just compliance` — прогон комплаенс‑сьюта.

---

## Изменения плана (лог)
- v0.1 — первичное заполнение по PLAN.md.
- v0.2 — добавлены canary/rollback и error‑budget.

