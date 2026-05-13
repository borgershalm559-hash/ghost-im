export interface IdentityStatusDto {
  exists: boolean;
  client_open: boolean;
}

export interface CreatedIdentityDto {
  ghost_id: string;
  fingerprint: string;
  display_name: string | null;
}

export interface ClientInfoDto {
  ghost_id: string;
  fingerprint: string;
  display_name: string | null;
  local_addrs: string[];
}

export interface ContactDto {
  ghost_id: string;
  fingerprint: string;
  display_name: string | null;
  local_alias: string | null;
  added_at: number;
  verified: boolean;
  pinned: boolean;
  muted: boolean;
  retention_seconds: number | null;
  last_message: string | null;
  last_message_at: number | null;
  last_message_direction: 'in' | 'out' | null;
  unread_count: number;
}

export interface MessageDto {
  uuid: string;
  direction: 'in' | 'out';
  content: string;
  sent_at: number;
  received_at: number | null;
}

export interface UpdateAvailableDto {
  version: string;
  notes: string | null;
  release_date: string | null;
}

export interface InviteDto {
  bech32: string;
  expires_at: number;
}

export interface InboundMessageEvent {
  from_ghost_id: string;
  content: string;
  received_at: number;
}

/** Retention preset values (seconds) shown in the dropdown. `null` = forever. */
export const RETENTION_PRESETS: { label: string; seconds: number | null }[] = [
  { label: 'Хранить всегда', seconds: null },
  { label: '30 дней', seconds: 30 * 24 * 3600 },
  { label: '7 дней', seconds: 7 * 24 * 3600 },
  { label: '24 часа', seconds: 24 * 3600 },
  { label: '1 час', seconds: 3600 },
  { label: '5 минут', seconds: 5 * 60 },
];
