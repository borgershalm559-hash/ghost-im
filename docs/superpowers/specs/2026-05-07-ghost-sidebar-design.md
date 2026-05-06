# Ghost Sidebar Redesign — дизайн

**Дата:** 2026-05-07
**Статус:** Draft (ожидает ревью пользователя)
**Автор:** Claude (на основе [Ghost.html design package](https://api.anthropic.com/v1/design/h/FcZ0QN1f2UrY1VOMrZ5iiQ))
**Целевая аудитория:** разработчики Ghost
**Реалистичный срок реализации:** 1-2 дня

---

## 1. Контекст и цели

### Что меняется

Текущий фронтенд (`/` = identity-карточка + InviteCard + AddContactForm + ContactList; `/chat/[ghost_id]` = отдельный полноэкранный чат с кнопкой «← На главную») заменяется **двухпанельной оболочкой в стиле Telegram/Discord**:

```
┌──────┬──────────────┬───────────────────────────────────┐
│ Rail │  Chat list   │  Chat pane (или empty state)      │
│ 80px │  360px       │  flex: 1                          │
│      │              │                                   │
│ All  │  Search      │  ┌─ Header (avatar+name+actions)─┐│
│ Per. │  ─────       │  │                               ││
│ Work │  ┌ row ┐     │  │  E2E banner                   ││
│ ...  │  │ Ren │     │  │  Messages...                  ││
│      │  │ Ash │     │  │                               ││
│      │  │ ... │     │  └───────────────────────────────┘│
│ Avt. │  └─────┘     │  ┌─ Composer ───────────────────┐ │
└──────┴──────────────┴───────────────────────────────────┘
```

Layout всегда виден. При клике на контакт меняется только содержимое правой panel'и. Когда контакт не выбран — там empty-state (большая ghost-иллюстрация + welcome + кнопки «Создать инвайт» / «Добавить контакт»).

### Goals

- Адаптация дизайна **V2 · Folder rail** из Ghost.html design package под наш Svelte/Tauri стэк, с пиксельной верностью к темной теме, типографике и spacing.
- Реальный chat list (не моки): живые контакты, последнее сообщение, время, unread-count.
- **Persistent shell**: переключение чатов не перерисовывает sidebar.
- Лёгкие фичи поверх MVP-1: theme toggle, pin, mute, verified flag, disappearing messages per contact, ghost-mode toggle, search-фильтр чатов.
- Empty state становится «домом»: identity, invite-генерация, add-contact живут в модалках, доступных оттуда.
- Релиз — **v0.0.4**, который протестирует auto-update path с уже установленного v0.0.3.

### Non-goals (deferred к более поздним планам)

| Фича | План |
|---|---|
| Tor / "over Tor" badge | MVP-3 |
| Online presence dot (через DHT) | план-04 follow-up |
| Voice / video calls | MVP-2 |
| Attachments (paperclip) | MVP-2 |
| Servers / channels | MVP-3 |
| Real-time typing indicator | MVP-2 |
| Reactions / edit / delete сообщений | MVP-2 |
| Folders CRUD (создание/удаление) | этот план кладёт **декоративный** rail; функциональные папки — отдельная итерация |
| Verification UI с safety-number + QR | этот план кладёт только bool-флаг «verified» (toggle в меню контакта); полный flow — отдельный план |

UI-элементы deferred-фич **не показываются вообще** (вместо fake-индикаторов). Принцип: в secure messenger честность визуала важнее «полноты» дизайна.

---

## 2. IA и маршрутизация

| Маршрут | До | После |
|---|---|---|
| `/onboarding` | onboarding | без изменений |
| `/` | identity + InviteCard + AddContactForm + ContactList | **shell + empty state** |
| `/chat/[ghost_id]` | отдельный экран | **тот же shell + ChatPane в main** |

`+layout.svelte` оборачивает всё:

```svelte
<UpdateBanner />
<ShellLayout>
  <slot /> <!-- правая панель: или EmptyState (на /) или ChatPane (на /chat/[id]) -->
</ShellLayout>
```

`ShellLayout` рендерит `<Rail />` + `<ChatList />` + `<slot />`. Rail и ChatList всегда видны, переключение URL **не размонтирует их** (используем layout-based persistent state).

**Удалённые маршруты:** ничего. **Удалённые компоненты:** `InviteCard.svelte`, `AddContactForm.svelte`, `ContactList.svelte` — заменяются модалками + `ChatList.svelte`.

---

## 3. Визуальная система

### Цвета (CSS-переменные на `:root[data-theme="dark"]` / `:root[data-theme="light"]`)

```css
/* dark — default */
--bg:           #0a0a10;
--surface:      #101019;
--elevated:     #171722;
--sidebar:      #0d0d14;
--rail:         #080810;
--border:       rgba(255,255,255,0.06);
--border-strong: rgba(255,255,255,0.10);
--text:         #e8e8f0;
--text-dim:     rgba(232,232,240,0.62);
--text-muted:   rgba(232,232,240,0.40);
--accent:       #9b8cff;
--accent-dim:   rgba(155,140,255,0.14);
--accent-soft:  rgba(155,140,255,0.22);
--success:      #3ddc97;
--danger:       #ff6b7a;
--bubble:       #1a1a26;
--bubble-mine:  linear-gradient(135deg, #6c5ce7 0%, #9b8cff 100%);
--hover:        rgba(255,255,255,0.04);
--selected:     rgba(155,140,255,0.10);

/* light */
--bg:           #f7f6f3;
--surface:      #ffffff;
--elevated:     #ffffff;
--sidebar:      #f1efeb;
--rail:         #ebe9e4;
--border:       rgba(0,0,0,0.06);
--border-strong: rgba(0,0,0,0.10);
--text:         #1a1a24;
--text-dim:     rgba(26,26,36,0.62);
--text-muted:   rgba(26,26,36,0.40);
--accent:       #6c5ce7;
--accent-dim:   rgba(108,92,231,0.10);
--accent-soft:  rgba(108,92,231,0.18);
--success:      #1a9968;
--danger:       #d83a4a;
--bubble:       #ffffff;
--bubble-mine:  linear-gradient(135deg, #6c5ce7 0%, #9b8cff 100%);
--hover:        rgba(0,0,0,0.04);
--selected:     rgba(108,92,231,0.08);
```

### Типографика

- Sans: **Inter** (400/500/600/700) — основной шрифт.
- Mono: **JetBrains Mono** (400/500) — для Ghost ID, fingerprint.
- Подключение: `@fontsource/inter` и `@fontsource/jetbrains-mono` через pnpm; импорты в layout.

### Размеры

- Rail width: 80px
- Chat list width: 360px
- Chat header height: 64px
- Avatar в row: 36px (dense)
- Avatar в чат-шапке: 40px
- Bubble max-width: 60% от ширины pane
- Border radius: 8px (cards), 10px (inputs), 12px (avatars), 16px (bubbles), 999px (pills)
- Borders: 0.5px (subtle)

---

## 4. Компоненты

Все под `frontend/src/lib/components/`.

### Layout

- **`ShellLayout.svelte`** — рендерит rail + chat-list + main-slot. Загружает контакты при старте, держит реактивный фильтр по folder + search-query, передаёт отфильтрованный список в ChatList. Подписан на inbound-события для авто-refresh.

### Rail

- **`Rail.svelte`** — 80px вертикальный rail. Кнопки:
  - All (active по умолчанию, единственная функциональная)
  - Personal / Work / Crypto / Channels / Burner / Archive — декоративные (`disabled`, `cursor: default`, opacity slightly reduced; никаких toast'ов, без обмана)
  - В самом низу — `Avatar` пользователя (clickable → открывает `ProfilePopover`)
- **`ProfilePopover.svelte`** — popover с настройками: Theme (radio dark/light), Ghost mode (toggle), кнопка «Show my Ghost ID» (открывает `IdentityModal`).

### Chat list panel

- **`ChatList.svelte`** — 360px панель:
  - Header: «All» (или название текущего фильтра) + count + edit-button (для будущего)
  - `<SearchBar />` — bind на ShellLayout.searchQuery
  - Прокручиваемый список:
    - Pinned секция (если есть закреплённые): метка «PINNED» + строки
    - Все остальные: метка «ALL CHATS» + строки
- **`ChatRow.svelte`** — props: `{ contact: ContactDto, selected: boolean }`:
  - `<Avatar />` (size: dense=36)
  - Полное имя (display_name или local_alias или fingerprint), с иконками: 🔒 если e2e (всегда true), 🛡 если verified, 🔕 если muted, 📌 если pinned
  - Время последнего сообщения справа
  - Last-message text (truncated, 1 строка)
  - Unread badge справа в нижней строке (если unread > 0)
  - Hover/selected подсветка (CSS из дизайна)
  - Контекст-меню (right-click) → `ContactMenu`
- **`SearchBar.svelte`** — input + иконка search; bind:value к шейру state.

### Chat pane

- **`ChatPane.svelte`** — рендерит:
  - `<ChatHeader />`
  - Прокручиваемая область сообщений:
    - `<EncryptionBanner />` — pill «Сообщения end-to-end зашифрованы»
    - Список `<MessageBubble />`'ов; группировка по дате (system message «6 May, 2026» каждый раз когда дата меняется)
    - При retention != null: вверху system-message «Disappearing messages: 24h» (или другой период)
  - `<Composer />`
- **`ChatHeader.svelte`** — props: `{ contact }`:
  - Avatar (40px)
  - Имя + verified-shield (если verified)
  - Под именем: fingerprint (моноширин) — единственная честная meta-инфа
  - Справа: кнопка «...» открывает `ContactMenu`
- **`MessageBubble.svelte`** — props: `{ msg, mine: boolean }`:
  - Если `mine`: справа, с purple gradient bg, asymmetric tail (top-right rounded меньше)
  - Если from contact: слева, neutral bg, asymmetric tail (top-left меньше)
  - Под пузырём: время + double-check (только для mine)
- **`Composer.svelte`** — textarea + кнопка send:
  - Round corner pill, dark bg, padding
  - Enter → отправка; Shift+Enter → перенос
  - Кнопка send с purple gradient, иконка send, glow-shadow
  - При busy: кнопка disabled, lightweight spinner
  - При error: красная строка под полем

### Empty state

- **`EmptyState.svelte`** — большая ghost-иллюстрация (SVG из дизайна, без изменений), под ней:
  - Заголовок: «Выберите чат, чтобы начать беседу»
  - Subtitle: «Каждое сообщение в Ghost зашифровано end-to-end и не оставляет следов на серверах.» (Tor-упоминание убрано — это пока неправда)
  - Status pills внизу: «E2E active» (green), «0 logs» (dim) — Tor-pill убран
  - Две CTA-кнопки: «Создать инвайт» → `InviteModal`, «Добавить контакт» → `AddContactModal`

### Модалки и попаперы

- **`Modal.svelte`** — generic backdrop + center pane + close-on-escape + close-on-backdrop-click.
- **`InviteModal.svelte`** — port текущего `InviteCard.svelte` в модалку (создать инвайт + textarea с bech32 + кнопка Copy).
- **`AddContactModal.svelte`** — port текущего `AddContactForm.svelte` в модалку (textarea для invite + Submit).
- **`IdentityModal.svelte`** — показывает full Ghost ID + fingerprint моноширинно + кнопка Copy.
- **`ContactMenu.svelte`** — popover-меню действий над контактом:
  - Pin/Unpin
  - Mute/Unmute
  - Mark as verified / Unmark
  - Retention dropdown: forever / 30d / 7d / 24h / 1h / 5min
  - (delete-контакт пока **не делаем** — деструктивно, требует confirmation, отдельной итерацией)

### Инфраструктура

- **`lib/icons.ts`** — экспортирует SVG-иконки как Svelte-функции (или `<script context="module">`-консты): search, plus, settings, pin, archive, shield, lock, send, mic, paperclip, emoji, hash, bell, bellOff, ghost, check, checkDouble, clock, fire, user, users, mute, chevDown, chevRight, folder, inbox, star, key, more. Stroke 1.6, viewbox 24×24, currentColor. Перевод 1-в-1 из `icons.jsx`.
- **`lib/theme.ts`** — экспорт CSS-переменных как объекта (для inline-стилей где нельзя переменными), `applyTheme(theme: 'dark'|'light')` пишет `document.documentElement.dataset.theme`.

---

## 5. Backend

### Migration 0003 — расширение contacts

Файл: `crates/ghost-storage/migrations/0003_contacts_extras.sql`

```sql
-- Per-contact settings for the redesigned UI: read tracking, pinning,
-- muting, retention. Verification was added in 0002 (verification INTEGER).

ALTER TABLE contacts ADD COLUMN last_read_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN muted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN retention_seconds INTEGER;  -- NULL = forever
```

(SQLite 3.35+ allows multiple `ALTER TABLE ADD COLUMN`. SQLCipher inherits.)

### Изменения repo (`crates/ghost-storage/src/repos/contacts.rs`)

Расширить `Contact` struct:

```rust
pub struct Contact {
    // existing fields...
    pub last_read_at: i64,
    pub pinned: bool,
    pub muted: bool,
    pub retention_seconds: Option<i64>,
}
```

Новые методы:

```rust
fn set_last_read_at(&self, ghost_id: &[u8], at: i64) -> Result<()>;
fn set_pinned(&self, ghost_id: &[u8], pinned: bool) -> Result<()>;
fn set_muted(&self, ghost_id: &[u8], muted: bool) -> Result<()>;
fn set_verified(&self, ghost_id: &[u8], verified: bool) -> Result<()>;
fn set_retention(&self, ghost_id: &[u8], seconds: Option<i64>) -> Result<()>;
```

### Изменения `messages.rs`

Новый метод (для unread badge):

```rust
fn unread_count(&self, contact_id: &[u8], since: i64) -> Result<i64>;
// SELECT COUNT(*) FROM messages WHERE contact_id = ? AND direction = 1 AND received_at > ?
```

### Изменения `ghost-client::Client`

`send_message`: при создании MessageRow выставлять `expires_at` если у контакта `retention_seconds.is_some()`:

```rust
let expires_at = contact.retention_seconds.map(|s| now() + s);
```

(сейчас в [client.rs:444](../../../crates/ghost-client/src/client.rs:444) и [client.rs:558](../../../crates/ghost-client/src/client.rs:558) хардкод `None`.)

### Background scrubber

В `Client::open()` спавнить tokio-задачу:

```rust
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        let _ = messages_repo.delete_expired(now());
    }
});
```

`messages_repo::delete_expired` уже в коде ([messages.rs:140](../../../crates/ghost-storage/src/repos/messages.rs:140)).

### Расширенный `ContactDto` (`crates/ghost-app/src/dto.rs`)

```rust
pub struct ContactDto {
    pub ghost_id: String,
    pub fingerprint: String,
    pub display_name: Option<String>,
    pub local_alias: Option<String>,
    pub verified: bool,
    pub pinned: bool,
    pub muted: bool,
    pub retention_seconds: Option<i64>,
    pub last_message: Option<String>,
    pub last_message_at: Option<i64>,
    pub last_message_direction: Option<String>, // "in" | "out"
    pub unread_count: i64,
}
```

`list_contacts` строит этот DTO внутри `ghost-app::commands::read::list_contacts`, JOIN'ом messages-таблицы или последовательными вызовами repo.

### Новые Tauri-команды

Файл: `crates/ghost-app/src/commands/contact_actions.rs`:

```rust
#[tauri::command]
pub async fn mark_chat_read(ghost_id: String, ...) -> CommandResult<()>;
#[tauri::command]
pub async fn set_pinned(ghost_id: String, pinned: bool, ...) -> CommandResult<()>;
#[tauri::command]
pub async fn set_muted(ghost_id: String, muted: bool, ...) -> CommandResult<()>;
#[tauri::command]
pub async fn set_verified(ghost_id: String, verified: bool, ...) -> CommandResult<()>;
#[tauri::command]
pub async fn set_retention(ghost_id: String, seconds: Option<i64>, ...) -> CommandResult<()>;
```

Файл: `crates/ghost-app/src/commands/settings.rs` (расширение существующего, или новый):

```rust
#[tauri::command]
pub async fn get_setting(key: String, ...) -> CommandResult<Option<String>>;
#[tauri::command]
pub async fn set_setting(key: String, value: String, ...) -> CommandResult<()>;
```

(Под капотом — существующий `SettingsRepo`.)

### Изменения `apps/ghost-desktop/src/main.rs`

Добавить новые команды в `tauri::generate_handler!`. Обновить capabilities (`capabilities/default.json`) — выдать allow на новые команды.

---

## 6. Тестирование

### Unit (Rust)

- `repos::contacts`: roundtrip get/set для каждого нового поля (pinned, muted, retention, last_read_at).
- `repos::messages::unread_count` — вставить mix in/out с разными received_at, проверить count после `since`.
- Migration 0003: применяется на чистой БД и на БД от plan-03 (idempotency не требуется, но миграции должны бежать строго один раз).

### Integration (Rust)

Расширить `crates/ghost-storage/tests/e2e_persistence.rs`:
- Тест: создать контакт с `retention_seconds = 5`, отправить сообщение, перевести часы (`now + 6s`), вызвать `delete_expired`, проверить что сообщение удалено.

Новый файл `crates/ghost-app/tests/contact_actions_e2e.rs`:
- Через `Client::open()` + commands: pin/unpin отражается в `list_contacts`. mark_chat_read обнуляет unread_count. set_retention переменно влияет на expires_at новых сообщений.

### Frontend (manual)

- `pnpm dev`: визуальный обход всех состояний (empty, with-contact-no-messages, with-contact-with-messages, при theme switch dark/light, при ghost-mode on/off, при pin/mute/verify, при retention=24h, при unread > 0).
- `cargo tauri build` + установить локально → визуально сверить с design package скриншотами (визуально опираемся на CSS из `variants.jsx`/`sidebar-parts.jsx`/`chat-panes.jsx`).

### Smoke (release)

- v0.0.4 публикуется через CI.
- v0.0.3 на машине пользователя обнаруживает обновление, signature верифицируется (форматы pubkey/sig теперь корректны), MSI устанавливается, identity сохранена.
- В обновлённом v0.0.4 — новый UI.

---

## 7. Rollout

1. Бранч `claude/sidebar-redesign` (или прямо в master, как идём весь plan-08).
2. Имплементация по плану (writing-plans → executing).
3. Локальный smoke (`pnpm dev`, потом `cargo tauri build`).
4. Bump `tauri.conf.json` 0.0.3 → **0.0.4**, `Cargo.toml` workspace.version → 0.0.4, `Cargo.lock` соответственно.
5. `git push origin master --tags` → Release workflow → release published.
6. **Пользовательский test:** v0.0.3 на машине должен показать баннер «Доступна Ghost 0.0.4», нажатие «Перезапустить» → MSI обновляется → запускается v0.0.4 с новым UI и сохранённой identity.

---

## 8. Открытые вопросы / решения по умолчанию

| Вопрос | Решение по умолчанию |
|---|---|
| Default theme при первой установке? | `dark` (как сейчас) |
| Default retention при создании контакта? | `NULL` (forever) — сохраняет текущее поведение |
| Default ghost_mode? | `false` (off) |
| Что делает toggle Ghost mode в MVP-1? | **Только визуал**: меняет цвет статуса в ProfileFooter с green «Online» на purple «Ghost mode · invisible» и кладёт ring на свой аватар. **Не меняет** behavior backend'а, потому что presence в MVP-1 не публикуется (см. MVP-1 audit). Подключим к реальному opt-out, когда заработает DHT-presence. |
| Группировка сообщений по дате — UTC или local? | **Local** (через `Intl.DateTimeFormat`). Junior дата в трасте, не в БД. |
| Куда деть кнопку «Settings» из ProfileFooter? | popover c radio + toggle + кнопка «Show my Ghost ID» |
| При клике на декоративные folder-tabs — что? | Ничего (cursor: default, opacity 0.5). Никаких toast'ов «coming soon». |
| Поддержка Russian locale? | Да, все строки на русском (как сейчас). i18n-инфраструктуры пока нет, hardcoded RU. |
| Группировка сообщений по дате? | Да, system-message «N MMM YYYY» при смене даты. |
| Подсветка ссылок / mentions в тексте сообщений? | Нет, plain text only (defer). |
| Markdown в сообщениях? | Нет (defer). |

---

## 9. Риски

- **Размер изменений во frontend.** Полный rewrite UI-слоя. Митигация: backend-изменения изолированы в новых repo-методах + новые команды; старые `addContact`/`createInvite`/`sendMessage`/`listMessages`/`listContacts` сохраняют контракт (только расширяется DTO, не ломается).
- **Cargo.lock конфликты при бампе версий.** Решено в plan-08: sed-апдейт всех `name = "ghost-..."` версий до 0.0.4.
- **Кеш auto-update.** v0.0.3 → v0.0.4 идёт через тот же путь, что мы только что выверили; форматы pubkey/sig правильные.
- **Тесты на Windows.** CI workflow `Test workspace` сейчас падает (не на наших правках); это **не блокер для релиза**, Release workflow тесты не гоняет. Отдельной задачей разобраться с упавшим тестом.

---

## 10. Связанные документы

- [Ghost MVP-1 design](2026-04-27-ghost-mvp1-design.md)
- [Plan 07 — Tauri App](../plans/2026-04-28-ghost-plan-07-tauri-app.md) (UI который этот документ заменяет)
- [Ghost.html design package](https://api.anthropic.com/v1/design/h/FcZ0QN1f2UrY1VOMrZ5iiQ) (исходный prototype)
- [docs/release-process.md](../../release-process.md) (как релизить v0.0.4)
