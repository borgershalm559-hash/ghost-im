# Ghost MVP-1 Plan 08 — Updater + Release Pipeline (дизайн)

**Дата:** 2026-04-29
**Статус:** Draft (ожидает ревью пользователя)
**Целевая аудитория:** разработчики Ghost
**Предшествующий контекст:** Plans 01-07 завершены (теги `plan-01-complete` … `plan-07-complete`). Тауринговое десктоп-приложение для Windows работает: онбординг, обмен инвайтами, E2EE-сообщения через GUI. Plan 08 — последний план MVP-1.

---

## 1. Контекст и цели

### Что осталось перед MVP-1 ship

После Plan 07 у нас есть рабочий desktop binary, но его нельзя обновлять без ручной замены `.exe`. Plan 08 закрывает эту дыру и собирает release pipeline, который превращает `git tag v0.0.X` в опубликованный signed binary в GitHub Releases с auto-update flow для уже установленных пользователей.

### Goals Plan 08

- `tauri-plugin-updater` интегрирован в `apps/ghost-desktop/`.
- Manifest schema (`latest.json`) определена, подписывается minisign-ключом в CI.
- Public minisign-ключ embedded в бинарник на build-time; verification работает end-to-end.
- GitHub Actions release workflow триггерится на `v*` тег: собирает .msi, подписывает, публикует Release.
- GitHub Actions CI workflow запускается на push/PR: fmt + clippy + test + frontend check.
- Reproducible build flags (~80% determinism для MVP-1).
- Real Windows icon (multi-size ICO заменяет 66B placeholder).
- Frontend: `UpdateBanner.svelte` показывает доступные обновления с кнопками "Перезапустить" / "Позже".
- Документация: `docs/release-process.md` runbook + `scripts/generate-minisign-keypair.sh` helper.

### Non-goals Plan 08 (отложено в Plan 09+)

- Windows EV Code Signing Certificate (~$300/год). SmartScreen warning при первом запуске остаётся.
- macOS Apple Developer Program ($99/год), notarization, `.dmg` сборки. macOS целиком отложен.
- Linux сборки (`.AppImage`/`.deb`) — пользователь явно запросил Windows-only.
- N-of-M signing (2 из 3 ключей, YubiKey backup). MVP-1: один offline minisign ключ.
- Sigstore Rekor transparency log entries.
- 100% reproducible builds через docker. MVP-1: ~80% через `--remap-path-prefix` + `SOURCE_DATE_EPOCH`.
- Свой домен (`updates.ghost.<...>`) и Cloudflare Pages. MVP-1: GitHub Releases как update channel.
- IPFS / Tor mirrors update channel.
- Settings экран (auto-download / notify-only / disabled toggle, частота проверок, кнопка "Проверить сейчас").
- `min_supported` поле в манифесте + UI про "Friend's version is too new".
- Update kill-switch / revocation list.
- Кастомный NSIS/WiX installer theme, license agreement screen.
- Landing page с download-кнопками.
- Homebrew tap, Scoop bucket, AUR.

### Threat model для Plan 08

Защищаемся от:
- Компрометация GitHub аккаунта без приватного ключа → атакующий не может выпустить malicious update (manifest signature не проверится). Может только blocking — публиковать невалидные релизы, но клиенты их отвергнут.
- Network MITM → tauri-plugin-updater использует HTTPS для GitHub API; signature на .msi проверяется поверх transport security.
- Downgrade attack → tauri-plugin-updater отвергает version ≤ текущей.

Не защищаемся от:
- Компрометация приватного minisign ключа (одна копия в GitHub Actions secret + локальный backup пользователя). Plan 09 хардит через N-of-M.
- Малициозный код в подписанной версии (если разработчик сам компрометнут или ошибся). Reproducible builds дают community верификацию: "shipped bytes = source code в репо".
- Side-channel против самого Tauri/WebView2 runtime — это уровень OS, вне scope.

---

## 2. Общая архитектура

```
Dev machine                         GitHub                          User machine
───────────                         ──────                          ────────────
1. git tag v0.0.2                   1. release.yml triggers         1. ghost-desktop running
   git push --tags                     on v* tag                       (e.g. v0.0.1)
                                    2. cargo tauri build (Win)      2. tauri-plugin-updater polls
                                    3. minisign manifest with          GH API at startup + every
                                       offline key (CI secret)         hour
                                    4. gh release create →          3. parses latest.json,
                                       upload .msi + latest.json       compares versions
                                                                    4. if newer: download .msi,
                                                                       verify minisign signature
                                                                       against embedded pubkey
                                                                    5. show toast: "Доступна
                                                                       Ghost X.Y.Z. Перезапустить?"
                                                                    6. on user confirm: replace
                                                                       binary, restart
```

**Trust anchor:** minisign pubkey embedded в бинарник на build-time через `tauri.conf.json` → `plugins.updater.pubkey`. Privatekey живёт ТОЛЬКО как GitHub Actions secret + локальный backup. Если GH-аккаунт компрометнут → атакующий может выпускать вредоносные обновления. Mitigation в Plan 09: N-of-M signing с offline YubiKeys.

**Где живёт `latest.json`:** GitHub Releases asset на каждом теге. Updater дёргает `https://api.github.com/repos/<owner>/ghost/releases/latest` (без своего домена).

**Update flow UX:** auto-check на launch + раз в час → silent download с прогресс-баром → toast "Доступна Ghost X.Y.Z. Перезапустить?" с кнопками "Перезапустить" / "Позже". Не auto-restart (пользователь знает когда удобно).

---

## 3. Компоненты и файлы

### Новые файлы

| Путь | Назначение |
|---|---|
| `apps/ghost-desktop/icons/icon.png` | 1024×1024 source PNG. Геометрический ghost silhouette, white-on-transparent. **Source-of-truth** — все остальные размеры авто-генерируются из неё через `cargo tauri icon icons/icon.png`. |
| `apps/ghost-desktop/icons/icon.ico` | **Заменить** 66B placeholder. Авто-генерируется из `icon.png`. |
| `apps/ghost-desktop/icons/32x32.png`, `128x128.png`, `128x128@2x.png`, `Square*.png` | Tauri-стандартный набор (auto-generated через `cargo tauri icon`). |
| `frontend/src/lib/components/UpdateBanner.svelte` | Тостовый баннер сверху main view. Показывается когда `tauri-plugin-updater` нашёл новую версию. |
| `crates/ghost-app/src/commands/updater.rs` | `check_for_update`, `download_and_install_update` Tauri commands. Тонкие обёртки над `tauri-plugin-updater` API. Возвращают status DTOs. |
| `.github/workflows/ci.yml` | Push/PR triggers. Запускает fmt + clippy + test + frontend check. |
| `.github/workflows/release.yml` | Tag push trigger. Билдит .msi + manifest + публикует GitHub Release. |
| `.cargo/config.toml` | Reproducible build flags: `RUSTFLAGS = "--remap-path-prefix=...=. -C debuginfo=0"`. |
| `docs/release-process.md` | Runbook: cut новый релиз шаг за шагом. |
| `scripts/generate-minisign-keypair.sh` | Helper для генерации `minisign.pub` (commit) + `minisign.key` (manual upload как GH secret `MINISIGN_PRIVATE_KEY`). |

### Изменяемые файлы

- `apps/ghost-desktop/Cargo.toml` — добавить `tauri-plugin-updater = { version = "2", features = ["native-tls"] }`.
- `apps/ghost-desktop/src/main.rs` — `.plugin(tauri_plugin_updater::Builder::new().build())` в Builder chain; новые команды в `invoke_handler!`.
- `apps/ghost-desktop/tauri.conf.json` — добавить `plugins.updater` секцию: `endpoints`, `pubkey`, `dialog: false`.
- `apps/ghost-desktop/capabilities/default.json` — добавить `updater:default`, `updater:allow-check`, `updater:allow-download-and-install`.
- `crates/ghost-app/src/dto.rs` — расширяется типом `UpdateAvailableDto { version, notes, release_date }`.
- `crates/ghost-app/src/commands/mod.rs` — `pub mod updater;`.
- `crates/ghost-app/src/lib.rs` — re-export для команд если требуется.
- `frontend/src/lib/types.ts` — `UpdateAvailableDto`.
- `frontend/src/lib/tauri.ts` — wrappers `checkForUpdate()`, `downloadAndInstallUpdate()`.
- `frontend/src/routes/+layout.svelte` — монтирует `<UpdateBanner />` поверх `{@render children()}`.
- `frontend/package.json` — добавить `@tauri-apps/plugin-updater = "^2"`.

### Намеренно не трогаем

- `crates/ghost-*` (кроме `ghost-app`) — никаких изменений в Rust core. Updater — это shell concern.
- `frontend/src/lib/state.svelte.ts` — update state живёт локально в `UpdateBanner`, не в global store.

**Total scope:** ~3 новых Rust файла, ~4 frontend файла, ~3 CI/config, плюс icons. Сравнимо с Plan 07 по объёму.

---

## 4. Поток данных (один update cycle)

**T-0** — User shipped binary v0.0.1; embedded pubkey; configured endpoint:
`https://api.github.com/repos/<owner>/ghost/releases/latest`

**T-1** — Dev cuts v0.0.2:
```bash
git tag -a v0.0.2 -m "release notes here"
git push origin v0.0.2
```

**T-2** — `release.yml` fires on tag push. На windows-latest runner:
- `cargo +1.87-x86_64-pc-windows-msvc install tauri-cli`
- `pnpm --dir frontend install --frozen-lockfile && pnpm build`
- `cargo tauri build` → `target/release/bundle/msi/ghost-desktop_0.0.2_x64_en-US.msi`
- `minisign -S -s $MINISIGN_KEY -m ghost-desktop_0.0.2_x64.msi` → `.msi.sig`
- Сборка `latest.json` через `jq` из `.msi.sig` + version + GitHub release URL.
- `gh release create v0.0.2 --notes-file release-notes.md ghost-desktop_*.msi latest.json`

**T-3** — Running v0.0.1 на user machine на app start + каждый час:
- `tauri-plugin-updater` GET'ает `https://api.github.com/repos/.../releases/latest`.
- Парсит JSON, находит `latest.json` среди assets, скачивает.
- Сравнивает: current `0.0.1` < manifest `0.0.2` ✓.
- Verifies minisign signature на .msi URL используя embedded pubkey. Invalid → silently abort, log warn.
- Эмитит `tauri://update-available` event.

**T-4** — Frontend `UpdateBanner` подписан на `update-available`. Показывает toast:
```
↑ Доступна Ghost 0.0.2     [Перезапустить] [Позже]
```

**T-5** — User clicks "Перезапустить":
- Frontend invokes `download_and_install_update` Tauri command.
- Plugin скачивает .msi (с progress events для больших файлов).
- Plugin re-verifies signature на скачанных байтах.
- Plugin запускает MSI installer (Tauri-стандартный WiX), exits old process.
- Installer launches new `ghost-desktop.exe`.

**T-6** — User clicks "Позже":
- Banner dismissed для текущей сессии.
- Следующий запуск перепроверяет и показывает снова если применимо.
- (Без "remind me later" таймера в MVP-1 — Plan 09.)

### Failure modes

| Сценарий | Поведение |
|---|---|
| Network down | Silent skip, retry next hour. |
| GH API 5xx / rate limit | Silent skip. |
| Signature invalid | ABORT install. Log warn. Юзер остаётся на v0.0.1. (Без UI alert в MVP-1.) |
| Downgrade attempt (manifest version ≤ current) | REFUSED плагином (built-in). Logged. |
| `min_supported > current` | Не имплементировано в MVP-1 (Plan 09). |

**Polling cadence:** on app start + every 1h. Configurable через env var для тестов.

---

## 5. Trust model

### Что подписывается

Только сами бинарники (`.msi`). Manifest schema:

```json
{
  "version": "0.0.2",
  "notes": "release notes here",
  "pub_date": "2026-04-29T12:34:56Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<minisign trusted comment + signed body>",
      "url": "https://github.com/<owner>/ghost/releases/download/v0.0.2/ghost-desktop_0.0.2_x64.msi"
    }
  }
}
```

`signature` — minisign-формат, подпись над **байтами .msi** (не над manifest целиком). Это стандарт `tauri-plugin-updater`: позволяет CI сначала собрать бинарник, подписать его, потом написать manifest. Manifest сам по себе не подписан — это OK потому что подпись внутри манифеста ссылается на конкретный SHA содержимого .msi, который дёргается по URL.

### Что НЕ подписывается (intentional MVP-1 ослабления)

- **Manifest сам по себе** — атакующий с MITM на github.com теоретически мог бы подменить `version` на меньшую и `signature` на пустую → плагин это ОТВЕРГНЕТ (signature mismatch). Подменить на больший version с валидной signature атакующему не удастся (нет приватного ключа). Manifest tampering = denial-of-update, не malicious update. Acceptable.
- **PE/Authenticode** (Windows code signing) — отсутствует. SmartScreen покажет "Unrecognized publisher" warning при первом запуске .msi. Юзер должен жмать "More info" → "Run anyway". Plan 09 добавляет EV cert (~$300/yr) → SmartScreen реагирует чисто.

### Где живут ключи

| Ключ | Где живёт | Кто имеет доступ |
|---|---|---|
| `minisign.pub` | Коммитится в `apps/ghost-desktop/` репо. Embedded в бинарник через `tauri.conf.json`. | Все. |
| `minisign.key` | GitHub Actions repo secret `MINISIGN_PRIVATE_KEY`. | Repo admins. |
| Резервная копия `minisign.key` | Local backup на dev-машине + secure storage (1Password / Bitwarden / etc на выбор разработчика). | Разработчик. |

### Если ключ компрометнут до Plan 09

- Атакующий может выпускать malicious updates всем установленным юзерам.
- Mitigation: ключ ротируется. Новый pubkey deploy'ится в новой версии. Старые юзеры на старом pubkey — manual re-install.
- В MVP-1 это acceptable risk потому что user count низкий и любая компрометация — уже catastrophe-уровня; не критично выжать N-of-M в первом релизе.

### Как Plan 09 хардит trust model

- N-of-M: 3 приватных ключа, 2 из 3 на YubiKey'ях, manifest требует ≥2 валидных подписей.
- Sigstore Rekor transparency log entry per release.
- Reproducible builds → community может пересобрать тэг и сравнить SHA-256.
- Win EV code-signing cert + macOS notarization.

---

## 6. CI pipeline + reproducible builds

### Два workflow

**`.github/workflows/ci.yml`** — на каждый push и PR. Не релизит, проверяет качество.

```yaml
on:
  push: { branches: [master] }
  pull_request:

jobs:
  rust:
    runs-on: windows-latest
    steps:
      - checkout
      - Setup Rust 1.87 + targets x86_64-pc-windows-msvc
      - Install Strawberry Perl (chocolatey)
      - cargo +1.87-x86_64-pc-windows-msvc fmt --all -- --check
      - cargo +1.87-x86_64-pc-windows-msvc clippy -p ghost-app -p ghost-desktop --all-targets -- -D warnings
      - cargo +1.87-x86_64-pc-windows-msvc test --workspace -- --test-threads=1

  frontend:
    runs-on: ubuntu-latest
    steps:
      - checkout
      - Setup Node 20 + pnpm
      - pnpm --dir frontend install --frozen-lockfile
      - pnpm --dir frontend check
      - pnpm --dir frontend build
```

**`.github/workflows/release.yml`** — на `v*` tag push.

```yaml
on:
  push: { tags: ['v*'] }

jobs:
  release-windows:
    runs-on: windows-latest
    steps:
      - checkout
      - Setup Rust + Strawberry Perl + Node + pnpm + tauri-cli
      - Compute SOURCE_DATE_EPOCH из commit timestamp
      - Export RUSTFLAGS из .cargo/config.toml
      - cargo +1.87-x86_64-pc-windows-msvc tauri build
      - Install minisign (chocolatey)
      - Sign .msi с $MINISIGN_PRIVATE_KEY → .msi.sig
      - Build latest.json через jq
      - gh release create $GITHUB_REF_NAME --notes-file release-notes.md ghost-desktop_*.msi latest.json
```

### Reproducible build flags

`.cargo/config.toml`:
```toml
[build]
rustflags = ["--remap-path-prefix", "${CARGO_MANIFEST_DIR}=.", "-C", "debuginfo=0"]
```

В `release.yml`:
```yaml
env:
  SOURCE_DATE_EPOCH: ${{ steps.commit-time.outputs.epoch }}
```

`SOURCE_DATE_EPOCH` влияет на mtime'ы внутри MSI (Tauri/WiX уважают эту env var). `--remap-path-prefix` убирает абсолютные пути из debug info. `debuginfo=0` гарантирует что debug-симолы не утекают в release бинарник.

**Что это даёт:** community может пересобрать тэг локально и проверить что `sha256(их.msi) == sha256(github-released.msi)`. Не доказательство safety (атакующий с private key всё ещё может подписать malicious build), но доказательство что **shipped bytes = source code в репо**.

**Что НЕ воспроизводимо в MVP-1:**
- Время сборки (rust компилятор вставляет timestamps в некоторых кейсах) — частично mitigated через `SOURCE_DATE_EPOCH`.
- Build environment (Rust 1.87 patches между minor versions) — mitigated через `rust-toolchain.toml` pin.
- Native dependencies (Strawberry Perl минор апдейтится) — acceptable.

Plan 09 хардит до 100% determinism через docker-based builds.

### Secrets configured в repo settings

| Secret | Origin |
|---|---|
| `MINISIGN_PRIVATE_KEY` | Generated через `scripts/generate-minisign-keypair.sh`. Содержимое `.key` файла. Загружается вручную через GH UI один раз. |
| `GITHUB_TOKEN` | Auto-provided GH Actions для `gh release create`. |

---

## 7. UI поверхность (`UpdateBanner.svelte`)

Минимальный компонент, монтируется в `+layout.svelte` поверх `{@render children()}`, чтобы был виден на любом маршруте (главная, чат, онбординг).

### Состояния

```
Состояние "новое обновление найдено":
┌─────────────────────────────────────────────────────────────┐
│ ↑ Доступна Ghost 0.0.2     [Перезапустить] [Позже]          │
└─────────────────────────────────────────────────────────────┘

Состояние "загружается":
┌─────────────────────────────────────────────────────────────┐
│ ↓ Скачивается обновление…  [████████░░░░░░░] 53%            │
└─────────────────────────────────────────────────────────────┘

Состояние "проверка подписи провалилась":
(только в логи в MVP-1, не UI)
```

### Поведение

- На `mount` баннер вызывает `checkForUpdate()` Tauri command один раз. Если `update == null`, баннер не показывается. Если `update != null`, ставит state `"available"` с `version` / `notes`.
- Параллельно подписывается на Tauri-событие `tauri://update-available` (плагин эмитит при background-проверках раз в час). На событие — обновляет state.
- Кнопка **"Перезапустить"** → вызывает `downloadAndInstallUpdate()` → state `"downloading"` с подпиской на `tauri://update-download-progress` → MSI installer запускается, текущий процесс умирает.
- Кнопка **"Позже"** → state `"dismissed"` для текущей сессии. Перепроверка при следующем запуске.
- Notes из manifest не показываются inline (узкая полоска); Plan 09 добавляет expandable "читать подробнее".

### Стиль

- Цвет: жёлто-янтарный фон (`#3a2e15` на тёмной теме) — заметно, но не алярмично.
- Высота: 48px, не перекрывает основной контент (родительский `<main>` получает `padding-top` когда баннер виден).
- Закрытие "Позже" анимируется fade-out 200ms.

### Что НЕ делает в MVP-1

- Не показывает changelog inline (Plan 09).
- Не разделяет "только проверка" / "загрузить позже" — всегда auto-download после "Перезапустить".
- Нет настроек частоты проверок — захардкожено 1 час.
- Нет ручной кнопки "Проверить сейчас".

---

## 8. Out of scope (явно отложено в Plan 09+)

**Доверие и подпись:**
- Windows EV Code Signing Certificate (~$300/год) — убирает SmartScreen warning.
- macOS Apple Developer Program ($99/год) + notarization.
- N-of-M signing (3 ключа, 2 на YubiKey'ях).
- Sigstore Rekor — публикация в transparency log.
- 100% reproducible builds через docker-based CI.

**Хостинг и инфра:**
- Свой домен (`updates.ghost.<...>`) и Cloudflare Pages.
- IPFS gateway как mirror update channel.
- Tor opt-in для проверки обновлений.

**Платформы:**
- macOS сборки (`.dmg`) — ждёт Apple Dev account.
- Linux сборки (`.AppImage`/`.deb`) — пользователь явно сказал "только Windows" в MVP-1.

**UX:**
- Settings экран с "auto-download / notify only / disabled / silent mode" toggle.
- Inline changelog в баннере.
- Toast про "проверка подписи провалилась".
- Toast про `min_supported > current`.
- "Remind me later" с таймером.

**Wire compat:**
- Поле `min_supported` в манифесте + UI про "Friend's version is too new".

**Анти-tamper:**
- Update kill-switch / revocation list.
- Manual rollback UI.

**Bundle polish:**
- Кастомные NSIS/WiX темы.
- License agreement screen в installer.
- Custom uninstall hooks.

**Distribution:**
- Landing page с download-кнопками.
- Homebrew tap, Scoop bucket, AUR.

---

## 9. Открытые вопросы (решено в этом дизайне)

| Вопрос | Решение |
|---|---|
| Уровень полноты реализации | "Soft scaffolding" — full e2e через GitHub Releases, без code signing. |
| Платформы в MVP-1 | Windows-only. Linux/macOS отложены. |
| Trust anchor | Single offline minisign key, GH Actions secret. N-of-M в Plan 09. |
| Update channel | GitHub Releases asset (`latest.json`). Своего домена нет. |
| Update flow UX | Auto-check + silent download + toast с "Перезапустить" / "Позже". |
| Polling cadence | On app start + 1 час. Configurable env var для тестов. |
| Какие иконки шипить | Real multi-size ICO (16/32/48/64/128/256), геометрический ghost silhouette. Source — одна 1024×1024 PNG, остальные генерируются через `cargo tauri icon`. |

---

## 10. Ссылки

- `tauri-plugin-updater` — https://v2.tauri.app/plugin/updater/
- `minisign` — https://jedisct1.github.io/minisign/
- Reproducible builds — https://reproducible-builds.org/
- GitHub Releases API — https://docs.github.com/en/rest/releases/releases
- Spec MVP-1 §7 (исходный source для Plan 08 design) — [docs/superpowers/specs/2026-04-27-ghost-mvp1-design.md](2026-04-27-ghost-mvp1-design.md)

---

**Конец дизайн-документа Plan 08.** Следующий шаг — implementation plan через `superpowers:writing-plans`.
