# Ghost MVP-1 — дизайн

**Дата:** 2026-04-27
**Статус:** Draft (ожидает ревью пользователя)
**Целевая аудитория:** разработчики Ghost
**Реалистичный срок реализации:** 3–4 месяца до shippable MVP-1

---

## 1. Контекст и цели

### Что такое Ghost (long-term vision)

Ghost — анонимный, защищённый десктоп- и мобильный-мессенджер, гибрид Discord и Telegram, не требующий хостинга со стороны разработчиков. Federated и end-to-end encrypted. Архитектура вдохновлена Matrix, но переписана с нуля и исправляет ключевые болячки Matrix (утечка метаданных через homeserver, тяжёлый Synapse, painful key management UX, плохая portability аккаунта).

Полный путь — 2–4 года поэтапной разработки (MVP-1 → MVP-2 → MVP-3 → MVP-4). Этот документ описывает только MVP-1.

### Что такое MVP-1

Минимальный десктоп-клиент, на котором проверяется фундамент — protocol, crypto, network, identity. Цель MVP-1 — доказать, что архитектура работает, чтобы дальше строить фичи (группы, серверы, голос, мобильные клиенты) на проверенной базе.

### Threat model (level 1–2)

Защищаемся от:
- Bad actors, провайдеров, утечек БД (level 1)
- Государственных запросов по ордеру в части содержимого (level 2)
- Метаданные защищены best-effort, не absolute

Не претендуем на:
- Защиту от nation-state APT (это level 3, потребовало бы пожертвовать почти всеми фичами Discord/Telegram-стиля)
- Анонимность от сетевого наблюдателя, который видит IP-трафик пользователя (требует Tor, опционально в MVP-3+)

### Goals MVP-1

- Один десктоп-клиент (Tauri) под Windows, macOS, Linux
- 1-на-1 текстовые E2EE-сообщения через MLS
- Создание идентичности офлайн, без email/phone/account
- Добавление контактов через invite-link / QR
- Локальное хранилище истории, шифрованное at-rest
- Базовый presence (online / offline / last-seen с opt-out)
- Системные нотификации ОС (не Apple/Google push)
- Auto-update со signed binaries

### Non-goals MVP-1 (отложено в MVP-2+)

- Группы, каналы, Discord-style серверы
- Voice / video
- Мобильные клиенты
- Multi-device sync
- Файлы (кроме inline-emoji в текст; крупные вложения — MVP-2)
- Tor wrapping (architectural placeholder, но не активирован)
- Production-grade федерация между разными homeserver'ами
- Полнотекстовый поиск
- Облачный бэкап (сознательное решение — облако = атака на anonymity)

---

## 2. Общая архитектура

### Embedded homeserver model

Главный fix Matrix-болячки: в Matrix юзер логинится на homeserver, который видит всё. У нас каждый клиент Ghost содержит встроенный homeserver внутри себя. Один процесс, один установщик, юзер не видит и не знает, что у него «сервер».

```
[Tauri webview UI]                                  ← пользователь
        ↕  (Tauri IPC, JSON-сообщения, in-process)
[ghost-client crate]
        ↕  (in-process function calls)
[ghost-server crate — embedded homeserver]
        ↕  (HTTP/3 over QUIC, TLS-бинд к Ghost ID)
[Friend's ghost-server crate]
        ↕  (in-process)
[Friend's UI]
```

Снаружи это выглядит как peer-to-peer; внутри — два homeserver'а федерируются один на один.

**Что даёт:**
- «Не требует хостинга» становится буквально правдой.
- Метаданные не утекают на чужой homeserver, потому что чужого homeserver нет.
- Архитектура с первого дня готова к федерации. Когда в MVP-3 появятся community-серверы (Discord-style), их можно будет хостить как standalone homeserver-бинари; embedded-клиенты с ними федерируются по тому же протоколу.

**Что требует:**
- Оба клиента онлайн для прямой доставки (Briar/Tox-модель). Оффлайн-доставка — MVP-2 (опциональные store-and-forward relays).
- NAT traversal — QUIC + hole punching + публичные STUN-ноды.

### Matrix-inspired, не Matrix-protocol

Ghost не совместим по wire с Matrix. Берём идеи (federation, room state, MLS migration), реализуем заново, чтобы исправить:
1. Метаданные — sealed sender + embedded homeserver
2. Тяжесть сервера — Rust + минимальный API (~5 endpoints против ~200 у Synapse)
3. Account portability — self-sovereign Ed25519 identity, переносится между homeserver'ами без потери истории
4. Key management UX — упрощённое до OS keystore + опциональный passphrase

---

## 3. Identity, keys, devices

### Иерархия ключей (3 слоя)

**Identity Key (IK) — мастер-идентичность**
- Ed25519 keypair, генерируется один раз на первом запуске
- Никогда не передаётся по сети в открытом виде
- Публичный IK = Ghost ID
- Recovery-сервера нет (сознательное решение для anonymous threat model). Потеря IK без бэкапа = безвозвратная потеря идентичности.

**Device Key (DK) — ключ конкретного устройства**
- Ed25519 keypair, генерируется при первом запуске
- Подписан IK (цепочка: IK signs DK signs ephemeral keys)
- В MVP-1 одно устройство, IK и DK живут вместе. Архитектурно разделены, чтобы multi-device в MVP-2 не был переписыванием.

**MLS state — пер-беседа**
- Управляется MLS-стейтмашиной (`openmls`) для каждой 1-на-1 беседы
- Pre-keys (X25519) — публикуются в embedded server, потребляются для async-handshake
- Leaf keys и group secrets — внутренняя кухня MLS

### Ghost ID

**Формат:** `ghost1` + bech32-encoded публичный IK. Пример: `ghost1qz4n9c5y8x2v7m3p6...` (~50 символов, error detection встроена в bech32).

**Короткий fingerprint** для verbal verification: BLAKE3 от full ID, отображается как 4 группы по 4 hex-символа: `1a2b-3c4d-5e6f-7890`.

**Display name** — опциональная UTF-8 строка, выставляется юзером, не аутентифицирована никем. В UI всегда рядом с fingerprint, чтобы impersonation был очевиден.

### Хранение секретов на диске

**Местоположение:**
- Linux: `~/.ghost/identity.encrypted`
- Windows: `%APPDATA%/Ghost/identity.encrypted`
- macOS: `~/Library/Application Support/Ghost/identity.encrypted`

**Шифрование at-rest:**
- Файл шифруется ключом, выведенным через Argon2id из:
  - Опциональный пользовательский passphrase (если задан)
  - OS keystore secret — Windows DPAPI / macOS Keychain / Linux Secret Service. Случайный 256-битный, генерируется один раз при установке.
- По умолчанию — OS keystore без passphrase (приоритет удобству). Юзер может включить passphrase в настройках.
- Шифр: XChaCha20-Poly1305.

### Onboarding flow

```
1. App стартует, identity-файла нет.
2. Welcome screen: "Создать идентичность?"
3. Юзер опционально: задаёт display name, опциональный passphrase.
4. Generate IK (Ed25519), DK (Ed25519, signed by IK).
5. Generate начальную пачку pre-keys (10 штук).
6. Encrypt identity → identity.encrypted.
7. Показать Ghost ID, предложить сделать backup.
8. Перейти в основной UI.
```

Никаких писем, телефонов, регистраций, server round-trips. Полностью офлайн.

### Backup / restore

**Export:** UI → «Settings → Export backup» → ввод passphrase → берём `identity.encrypted` + `ghost.db`, упаковываем в zip, шифруем age-форматом (XChaCha20-Poly1305 + Argon2id из passphrase) → сохраняем как `ghost-backup-<date>.age`.

**Restore (на новой машине):** UI → «Restore backup» → указать .age файл + passphrase → распаковка в стандартные пути → запуск normal flow.

**Если бэкапа нет и устройство утеряно:** идентичность потеряна. Контакты увидят, что Ghost ID больше не отвечает. Чтобы вернуться — новый Ghost ID, заново добавить себя контактам.

### Display name и защита от impersonation

В UI всегда: `Alice (1a2b-3c4d-5e6f-7890)`. При первом контакте показываем full Ghost ID и предлагаем «verify out-of-band» (Signal-style safety number). Если safety number меняется — warning «контакт перевыпустил ключи».

---

## 4. Crypto и protocol layer

### Криптопримитивы

| Назначение | Алгоритм | Библиотека |
|---|---|---|
| Подписи (IK, DK) | Ed25519 | `ed25519-dalek` |
| Key exchange | X25519 | `x25519-dalek` |
| Симметричное шифрование | XChaCha20-Poly1305 | `chacha20poly1305` |
| Хеширование | BLAKE3 | `blake3` |
| KDF (passphrase-derived) | Argon2id | `argon2` |
| KDF (key-derived) | HKDF-SHA-256 | `hkdf` |
| E2EE-протокол | MLS (RFC 9420) | `openmls` |
| Сериализация wire format | Canonical CBOR (RFC 8949) | `ciborium` или `serde_cbor` |

**Никакой кастомной криптографии.** Всё — published, audited, IETF-стандартизированные блоки.

### Почему MLS, а не Signal Double Ratchet

- IETF-стандарт RFC 9420 (2023), на него мигрируют Wickr, Wire, Matrix.
- Forward secrecy + post-compromise security для групп до тысяч членов через TreeKEM.
- В MVP-1 используем MLS-группу размера 2 для 1-на-1. Когда дойдём до групп в MVP-2 — та же машина, не переписывание.

### Pre-keys и асинхронный first contact

Чтобы Alice могла начать переписку с Bob, когда Bob офлайн:

1. Embedded server публикует пачку **KeyPackages** на endpoint `GET /v1/keypackages/<ghostid>`.
2. Каждый KeyPackage: device identity (DK), one-time prekey, init key, capabilities, signature.
3. Alice фетчит один (consumed после use).
4. Alice локально создаёт MLS-группу из себя и Bob, генерирует Welcome message.
5. Welcome доставляется в Bob's inbox (direct connect когда Bob онлайн; в MVP-2 — через mailbox).
6. Bob процессит Welcome, MLS-группа materialises у него.

**Pre-key replenishment:** при запуске Bob's server проверяет остаток и догенеривает до 10 штук.

**Защита от key-package exhaustion:** если pre-keys исчерпаны — fallback на **last-resort key** (не one-time, доверяет, что одного использования с одним контактом достаточно для начального handshake; дальше MLS делает proper ratchet).

### Sealed sender

Fix Matrix-болячки #1: homeserver не видит, кто отправил сообщение.

**Как работает:**
- Каждый юзер публикует delivery key (X25519, derived из IK) на `GET /v1/delivery-key/<ghostid>`.
- При отправке Alice → Bob:
  1. Шифрует MLS-payload (содержит её sender index в группе).
  2. Оборачивает: `{ inner_sender_id: Alice, mls_ct: <...> }` шифруется к Bob's delivery key.
  3. Внешний envelope: `{ recipient: Bob, timestamp_rounded, sealed_blob: <...> }` отправляется.
- Bob's server видит только: «кто-то прислал Bob сообщение в момент T». Не видит Alice ID, не видит content.
- Bob расшифровывает sealed_blob → видит inner_sender_id → процессит MLS → видит content.

В MVP-1 семантика частично избыточна (Bob's server и есть Bob), но реализуем сразу — иначе при добавлении relays/федерации придётся переписывать wire format.

### Wire message format

```
OuterEnvelope {
  version: u8                    // protocol version (для backwards compat)
  msg_type: u8                   // 0=app_message, 1=mls_handshake, 2=ack, ...
  recipient: GhostID             // 32 байта raw Ed25519 pubkey
  timestamp: u64                 // округлённый до секунды UTC
  sealed_blob: Bytes             // ciphertext, ниже
}

// Integrity outer-полей обеспечивается TLS 1.3 (QUIC) в транзите;
// неправильный recipient автоматически отбрасывается, потому что
// sealed_blob расшифровывается только delivery key соответствующего получателя.

SealedBlob (encrypted to recipient's delivery key) {
  sender_id: GhostID             // настоящий отправитель
  payload_type: u8               // text | mls_app | mls_proposal | mls_commit | ...
  payload: Bytes                 // если MLS-сообщение — это MLSMessage (RFC 9420 §6)
  msg_uuid: 16 bytes             // dedup на стороне получателя (UUID v7, time-ordered)
  sender_signature: 64 bytes     // Ed25519(DK, hash(sender_id || payload_type || payload || msg_uuid))
}
```

Сериализация — canonical CBOR.

### Гарантии безопасности

- **Confidentiality:** контент знают только участники беседы.
- **Forward secrecy:** компрометация ключей сегодня не раскрывает прошлые сообщения.
- **Post-compromise security:** компрометация сегодня не раскрывает БУДУЩИЕ сообщения после следующего ratchet.
- **Authenticity:** каждое сообщение signed Device Key, Bob отвергает при невалидной подписи или inconsistent MLS state.
- **Replay protection:** MLS epoch + msg_uuid dedup.
- **Sender anonymity vs server:** sealed sender прячет отправителя.
- **Не обещаем:** анонимность от network observer, видящего IP (требует Tor); защита от malicious endpoint, который тебя обманывает (только out-of-band fingerprint verification).

### Safety numbers

После первого handshake — общий 60-значный safety number (BLAKE3 от sorted concat обоих IK + group epoch info), показанный как 5 групп по 5 цифр.

UI flow: кнопка «Verify safely» в карточке контакта → показывает safety number + QR → юзеры созваниваются по другому каналу, сравнивают → «Mark verified» → зелёная галочка. Изменение safety number → warning.

---

## 5. Transport, networking, discovery

### Транспортный стек

**Клиент ↔ embedded server:** Tauri IPC (in-process).

**Embedded server ↔ Embedded server (между машинами):**
- HTTP/3 over QUIC через `quinn` (Rust QUIC, проверена в production)
- Fallback на HTTP/1.1+TLS over TCP для сетей с заблокированным UDP
- TLS 1.3 встроен в QUIC

### TLS-сертификаты — кастомное доверие

- Каждый Ghost server генерит self-signed TLS-сертификат на старте, подписанный его DK
- В SAN записан Ghost ID
- Клиент НЕ проверяет цепочку через CA (CA-системе не доверяем)
- Клиент проверяет: pubkey TLS-сертификата ↔ ожидаемый GhostID. Если не совпадает — connection rejected.
- Это делает MITM невозможным без компрометации IK получателя.

### HTTP API embedded server (MVP-1)

```
GET  /v1/version                  → { protocol: "ghost/1", min_compat: "ghost/1" }
GET  /v1/keypackages/{ghostid}    → один MLS KeyPackage (consumed)
GET  /v1/delivery-key/{ghostid}   → текущий X25519 delivery pubkey
POST /v1/inbox                    → принять sealed envelope (bytes), вернуть 200
GET  /v1/presence/{ghostid}       → { online: bool, last_seen: u64 } (если подписан)
```

Всего ~5 endpoints. Никакого Synapse-overhead на 200+ endpoints.

### Discovery: GhostID → endpoint

**Kademlia DHT через `rust-libp2p`** (production-ready, IPFS строится на нём).

- DHT-ключ = BLAKE3(IK)
- Каждый клиент публикует **AddressRecord** в DHT каждые ~5 минут:
  ```
  AddressRecord {
    ghostid: <IK>
    endpoints: [ "ip:port", "ip:port" ]    // IPv4 + IPv6
    expires_at: u64                         // now + 10 минут
    signature: Ed25519(IK, ...)
  }
  ```
- Друг ищет по BLAKE3(IK), получает свежий record, использует endpoint.

**Bootstrap DHT:** хардкодим список из ~5 публичных bootstrap-нод (можно использовать существующие IPFS bootstrap nodes — они работают для libp2p вообще). Юзер может добавить свои в настройках.

**Privacy concern:** DHT-наблюдатели видят, какие IK ищут. Это утечка метаданных на уровне «X пытается связаться с Y». В MVP-1 не закрываем; документируем честно. Митигация — Tor-wrapping в MVP-3+.

### NAT traversal

По убывающей надёжности:
1. QUIC UDP hole punching через libp2p AutoNAT/DCUtR
2. Публичные STUN-сервера (Google, Cloudflare) — узнаём external endpoint
3. Если symmetric NAT / hostile firewall → graceful failure: «Не получается напрямую соединиться, попробуй другую сеть». TURN-relay добавится в MVP-2.

По опыту libp2p в IPFS, ~85% домашних юзеров получают direct connection.

### First contact flow

**Alice создаёт invite:**
```
GhostInvite {
  ghostid: <IK>
  hint_endpoints: [ "current ip:port" ]
  invite_token: <16 random bytes>
  expires_at: u64                         // now + 7 дней
  signature: Ed25519(IK, ...)
}
```
Кодируется в bech32 (`ghostinvite1q...`) + QR.

**Bob принимает:**
1. Парсим, получаем GhostID + endpoint hint.
2. Resolve endpoint: hint → fallback DHT lookup.
3. QUIC connection, TLS handshake с проверкой cert→GhostID.
4. `GET /v1/keypackages/<alice_ghostid>` → KeyPackage.
5. Локально создаём MLS-группу размера 2, генерируем Welcome.
6. `POST /v1/inbox` к Alice — sealed envelope с Welcome + invite_token.
7. Alice's server проверяет invite_token, передаёт Welcome в client.
8. UI Alice: «Bob added you. Accept?» По умолчанию требуется явное подтверждение (anti-stalking). Глобальная настройка «Auto-accept incoming invites» в Settings отключена по умолчанию; включается осознанно.
9. Accept → MLS-группа активна, обмен сообщениями.

**Если Alice офлайн в момент scan:** Bob's outbox кэширует Welcome, retry экспоненциально (30s, 1min, 5min, 30min, 1h), пока Alice не появится онлайн или invite не expires (7 дней).

### Presence

- Online → клиент публикует **PresenceRecord** в DHT каждые ~60 секунд:
  ```
  PresenceRecord {
    ghostid: <IK>
    online: true
    last_seen: u64
    expires_at: u64   // now + 90s
    signature: Ed25519(IK, ...)
  }
  ```
- Контакты поллят DHT раз в 30 секунд.
- Юзер может отключить публикацию (станет «invisible»). По умолчанию presence публикуется только верифицированным контактам, others видят «unknown».
- Last-seen опционально, отдельный тоггл.

### Tor-готовность (architectural placeholder)

Транспортный слой абстрагирован за trait `Transport`. В MVP-1 — `QuicTransport`. В будущем — `TorTransport` (Ghost становится onion service). Onion address выводится детерминированно из IK (стандартный v3 derivation), Ghost ID единственный в обоих режимах.

---

## 6. Local storage

### Технологический выбор

**SQLite через `sqlx` (Rust async) + SQLCipher** для шифрования файла at-rest.

Почему:
- SQLite: zero-config, single-file, ACID, протестирован в миллиардах устройств.
- `sqlx`: compile-time SQL checking, async, миграции.
- SQLCipher: AES-256 на уровне страниц БД, прозрачно для прочего кода.

**Master DB key:** выводится через HKDF-SHA-256 из приватных байтов IK + статической соли `"ghost.db.encryption.v1"`. Привязка к identity — теряешь identity-файл, теряешь и БД.

Поток открытия БД при запуске: разблокировать `identity.encrypted` (через OS keystore + опциональный passphrase) → получить IK → derive master DB key → `sqlx` открывает БД с этим ключом.

**Расположение:** рядом с identity — `~/.ghost/ghost.db`, `%APPDATA%/Ghost/ghost.db`, `~/Library/Application Support/Ghost/ghost.db`.

### Схема БД (MVP-1)

```sql
-- Контакты юзера
CREATE TABLE contacts (
  ghost_id        BLOB PRIMARY KEY,           -- 32 байта raw IK
  display_name    TEXT,                       -- то, что юзер сам выставил себе
  local_alias     TEXT,                       -- то, как ты его переименовал у себя
  fingerprint     TEXT NOT NULL,              -- "1a2b-3c4d-..."
  added_at        INTEGER NOT NULL,
  last_endpoint   TEXT,                       -- последний known IP:port (cache)
  verification    INTEGER NOT NULL DEFAULT 0, -- 0=unverified, 1=verified
  notes           TEXT,
  blocked         INTEGER NOT NULL DEFAULT 0
);

-- MLS group state (одна группа на 1-на-1 беседу)
CREATE TABLE mls_groups (
  group_id        BLOB PRIMARY KEY,           -- MLS group ID (32 байта)
  contact_id      BLOB NOT NULL,
  state_blob      BLOB NOT NULL,              -- сериализованный openmls group state
  current_epoch   INTEGER NOT NULL,
  created_at      INTEGER NOT NULL,
  last_updated    INTEGER NOT NULL,
  FOREIGN KEY (contact_id) REFERENCES contacts(ghost_id)
);

-- Опубликованные нами KeyPackages
CREATE TABLE my_keypackages (
  package_id      BLOB PRIMARY KEY,
  package_blob    BLOB NOT NULL,
  private_key     BLOB NOT NULL,              -- private init key (для consume)
  created_at      INTEGER NOT NULL,
  consumed_at     INTEGER,                    -- NULL если ещё доступен
  is_last_resort  INTEGER NOT NULL DEFAULT 0
);

-- Сообщения
CREATE TABLE messages (
  msg_uuid        BLOB PRIMARY KEY,           -- 16 байт UUID v7 (time-ordered)
  contact_id      BLOB NOT NULL,
  direction       INTEGER NOT NULL,           -- 0=outgoing, 1=incoming
  content_type    INTEGER NOT NULL,           -- 0=text, 1=system_event, ...
  content         TEXT NOT NULL,              -- plaintext (БД encrypted)
  sent_at         INTEGER NOT NULL,
  received_at     INTEGER,
  status          INTEGER NOT NULL DEFAULT 0, -- 0=pending, 1=sent, 2=delivered, 3=read, 4=failed
  reply_to        BLOB,                       -- msg_uuid реплая
  expires_at      INTEGER,                    -- для disappearing messages
  FOREIGN KEY (contact_id) REFERENCES contacts(ghost_id)
);

CREATE INDEX idx_messages_contact_time ON messages(contact_id, sent_at);
CREATE INDEX idx_messages_expires ON messages(expires_at) WHERE expires_at IS NOT NULL;

-- Outbox: исходящие, ждущие доставки
CREATE TABLE outbox (
  msg_uuid        BLOB PRIMARY KEY,
  recipient_id    BLOB NOT NULL,
  envelope_blob   BLOB NOT NULL,
  attempts        INTEGER NOT NULL DEFAULT 0,
  next_retry_at   INTEGER NOT NULL,
  last_error      TEXT
);

-- Inbox dedup
CREATE TABLE inbox_dedup (
  msg_uuid        BLOB PRIMARY KEY,
  received_at     INTEGER NOT NULL
);

CREATE INDEX idx_inbox_dedup_time ON inbox_dedup(received_at);

-- Настройки приложения
CREATE TABLE settings (
  key             TEXT PRIMARY KEY,
  value           TEXT NOT NULL
);

-- Schema version (для миграций)
CREATE TABLE schema_version (
  version         INTEGER PRIMARY KEY,
  applied_at      INTEGER NOT NULL
);
```

**Не в БД (in-memory only):**
- Presence cache контактов — ephemeral, поллится из DHT
- DHT routing table — у libp2p свой mini-store
- Network connection state

При cold start contact's presence = «unknown», пока следующий DHT poll не подтянет.

### Миграции

`sqlx-migrate`. Файлы в `migrations/`, нумерованы (`0001_init.sql`, `0002_*.sql`, ...). При запуске: read schema_version → apply unapplied в транзакции → bump версию. Downgrade-only (юзер откатил app) → fail с понятным сообщением.

### Disappearing messages

UI: per-contact retention — `forever | 30d | 7d | 24h | 1h | 5min`. Default `forever`.

Когда не `forever`: при записи `expires_at = now + retention`. Background-таск раз в минуту: `DELETE WHERE expires_at < now`. Соответствующие outbox/dedup также cleanup.

В UI явно: «You have 7d retention; Alice has forever — they may keep messages longer». Полу-разделяемая договорённость, не enforceable.

### Логирование

- `tracing` (Rust). По умолчанию WARN+ in `~/.ghost/logs/ghost.log` (rotate 7 дней, 5 MB).
- Опция «debug logging» в настройках.
- **Не попадает в логи:** content сообщений, ключи, identity, IP контактов.
- Юзер может скопировать логи в bug-репорт; авто-телеметрия не собирается.

---

## 7. Modules, testing, build, release

### Cargo workspace layout

```
ghost/
├── Cargo.toml                  ← workspace
├── crates/
│   ├── ghost-core/             ← общие типы, ошибки, GhostID, fingerprint
│   ├── ghost-identity/         ← IK/DK, identity.encrypted, OS keystore
│   ├── ghost-protocol/         ← wire format (CBOR), envelopes, sealed-sender, MLS-обёртка над openmls
│   ├── ghost-storage/          ← SQLite+SQLCipher, миграции, репозитории
│   ├── ghost-network/          ← QUIC через quinn, libp2p DHT, NAT traversal, trait Transport
│   ├── ghost-server/           ← embedded HTTP-сервер, /v1/* endpoints, dispatcher
│   ├── ghost-client/           ← orchestration: send/receive flow, contact mgmt, presence
│   ├── ghost-updater/          ← Tauri updater wiring, signature verification
│   └── ghost-app/              ← Tauri commands, IPC мост к фронтенду
├── apps/
│   └── ghost-desktop/          ← Tauri-shell бинарь
├── frontend/                   ← SvelteKit
├── migrations/
│   └── 0001_init.sql, ...
├── tests/e2e/                  ← полные E2E (два процесса, real handshake)
└── scripts/release/            ← reproducible build, signing, manifest gen
```

**Зависимости направлены вниз:** `ghost-app` → `ghost-client` → `ghost-server`/`ghost-storage`/`ghost-network` → `ghost-protocol` → `ghost-identity` → `ghost-core`. Циклов нет, проверяется через `cargo-deny`.

### Error handling

- Внутри библиотечных crate: `thiserror`, typed enum errors, каждая crate экспортирует свой `Error` + `Result<T>`.
- На app boundary (Tauri commands, main): `anyhow` для конверсии в человеко-читаемые.
- Никогда не паникуем в production-коде кроме true invariants.
- Ошибки сети vs ошибки протокола разделены (первое — retry-able, второе — нет).
- Перед logging — scrubbing PII / metadata.

### Тестовая стратегия

| Уровень | Что покрывает | Tooling |
|---|---|---|
| Unit | Чистая логика внутри crate (parsing, KDF, схемы) | std `#[test]` |
| Integration | Несколько crate (e.g., storage + protocol round-trip) | `tests/` per crate |
| Property-based | Crypto-граничные случаи, парсеры | `proptest` |
| Fuzz | Парсеры wire format (CBOR), MLS-входы | `cargo-fuzz` |
| E2E | Два процесса Ghost, real network, full handshake → exchange → verify | tokio test harness |

**Цель покрытия:** 80%+ на crate-level (`cargo-tarpaulin`). E2E — самый ценный сигнал для federated протокола.

**Регрессионные security-тесты:**
- TLS-cert не bound к ожидаемому GhostID → connection rejected.
- Replayed envelope (тот же msg_uuid) → отброшен.
- Подделанная подпись AddressRecord в DHT → discarded.
- Bad signature в update manifest → refuse update.
- Downgrade attempt в updater → refused.

### Build pipeline (CI)

GitHub Actions, на каждый push:
1. `cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings`
2. `cargo test --workspace`
3. `cargo deny check` (license, advisory, banned crates)
4. Frontend: `pnpm build` + `pnpm test`
5. Tauri build на трёх раннерах (Windows, macOS, Linux)

На git-tag (`v0.X.Y`):
- Всё выше +
- Reproducible build flags: `RUSTFLAGS="--remap-path-prefix=$PWD=. -C debuginfo=0"`, sorted dependencies, fixed timestamps
- Code signing
- Manifest generation для updater
- GitHub Release + push manifest на update server

### Code signing

| Платформа | Что нужно | Стоимость |
|---|---|---|
| Windows | EV Code Signing Certificate (Sectigo / DigiCert) для SmartScreen | ~$300/год |
| macOS | Apple Developer Program + notarization | $99/год |
| Linux | Опциональная GPG-подпись `.AppImage`/`.deb` | $0 |

### Update channel

**Endpoint:** `https://updates.ghost.<domain>/v1/<channel>/latest.json` (static-hosted, Cloudflare Pages / любой CDN). Mirror — GitHub Releases (всегда), опционально IPFS gateway. Tauri-updater пробует mirrors по порядку.

**Манифест:**
```json
{
  "version": "0.5.2",
  "released_at": 1735689600,
  "min_supported": "0.4.0",
  "platforms": {
    "windows-x64": { "url": "...", "sha256": "...", "size": 24567890 },
    "macos-arm64": { "url": "...", "sha256": "...", "size": ... },
    "linux-x64":   { "url": "...", "sha256": "...", "size": ... }
  },
  "signatures": [
    { "key_id": "release-key-1", "sig": "..." },
    { "key_id": "release-key-2", "sig": "..." }
  ],
  "transparency_log_entry": "https://rekor.../entry/abc"
}
```

**N-of-M подписи:** 3 release keys (один на офлайн-машине, два на YubiKey-ах разных мейнтейнеров). Минимум 2 подписи валидируется на клиенте. Public keys всех трёх embedded в приложение на build time.

**Дополнительная защита:**
- Monotonic version: клиент отказывается ставить ≤ текущей.
- Binary transparency: каждый manifest публикуется в public append-only log (Sigstore Rekor).
- Reproducible builds: community может пересобрать тэг и сравнить SHA-256.
- Tor opt-in для проверки обновлений (paranoid users).

**UX:**
- Async background check при запуске.
- Если есть → silent download.
- Когда скачано → toast: «Доступна Ghost X.Y.Z. Перезапустить?» с кнопками `Restart now` / `Later`.
- `Later` → применится при следующем запуске.
- Накопление 3 обновлений за неделю простоя → клиент применит самое свежее (через chain).

**Wire protocol compat:**
- `version: u8` в envelope.
- `/v1/version` → `{ protocol, min_compat }`.
- Если peer's `min_compat > my_protocol` → UI: «Friend's version is too new, please update».
- Major bumps — отдельная mainline; minor — back-compat несколько релизов.

**Настройки auto-update (UI):**
- Default: auto-check + download + notify.
- Silent mode: применяется без уведомления.
- Notify only: проверять, не скачивать.
- Disabled: полностью manual.

### Distribution

- Landing page с download-кнопками. Без analytics, без cookies.
- GitHub Releases с прикреплёнными signed binaries.
- Package managers (после MVP-1): Homebrew tap, Scoop bucket, AUR.

---

## 8. Risks и open questions

### Технические риски MVP-1

| Риск | Митигация |
|---|---|
| Зависимость от libp2p и IPFS bootstrap nodes для discovery | Документируем явно, юзеры могут добавлять свои bootstrap-ноды. Долгосрочно — рассмотреть приватные DHT для Ghost. |
| ~15% юзеров за symmetric NAT не смогут соединиться | В MVP-1 — graceful failure с подсказкой. В MVP-2 — TURN-relay. |
| DHT публикует, что юзер кого ищет (метаданные) | Принимаем как level 1-2 компромисс. Tor-wrapping в MVP-3+. |
| openmls — единственная зрелая Rust-имплементация MLS | Зависимость рискованная, но альтернатива (свой MLS) — годы работы. |
| SQLCipher — C-зависимость, build complexity cross-platform | Принимаем — security важнее convenience. CI настраивает build matrix. |
| Code signing infra (~$400/год) — обязательное расходование | Да, без этого SmartScreen/Gatekeeper warnings отпугивают юзеров. |

### Open questions (решаются после MVP-1)

- **Open-source license:** рекомендация AGPL-3.0 (защита от закрытых форков). Решение пользователя.
- **Frontend framework:** рекомендация SvelteKit (~30KB runtime). Альтернативы — React, Solid.
- **Branding/UI design:** отдельная UX-brainstorm-сессия после того, как core работает.
- **Domain для updates и landing:** нужен (например, `ghost-im.app` / `getghost.io`). Cloudflare-аккаунт для CDN.
- **Юридическая структура** (если планируется монетизация / донаты / выпуск): отдельная тема.

---

## 9. Out of scope (план будущих MVP)

- **MVP-2 (~5–7 мес после MVP-1):** small groups (~50 чел), multi-device sync, базовый file transfer, optional store-and-forward mailbox для оффлайн-доставки, TURN-relay для NAT-трудных сетей, FTS5 поиск.

- **MVP-3 (~8–12 мес после MVP-2):** federation между разными homeserver'ами, Discord-style community-серверы с каналами и ролями, Telegram-style broadcast-каналы, опциональный Tor wrap.

- **MVP-4 (~6–12 мес после MVP-3):** voice/video через WebRTC, мобильные клиенты (iOS + Android), UX-полировка, сторонний security audit.

---

## 10. Ссылки

- RFC 9420 — The Messaging Layer Security (MLS) Protocol
- RFC 8949 — Concise Binary Object Representation (CBOR)
- RFC 9000 — QUIC: A UDP-Based Multiplexed and Secure Transport
- `openmls` — https://github.com/openmls/openmls
- `rust-libp2p` — https://github.com/libp2p/rust-libp2p
- `quinn` (QUIC) — https://github.com/quinn-rs/quinn
- `sqlx` — https://github.com/launchbadge/sqlx
- SQLCipher — https://www.zetetic.net/sqlcipher/
- Tauri — https://tauri.app
- Sigstore Rekor (transparency log) — https://docs.sigstore.dev/rekor/

---

**Конец дизайн-документа MVP-1.** Следующий шаг — implementation plan через `superpowers:writing-plans`.
