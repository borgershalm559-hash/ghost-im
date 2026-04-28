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
}

export interface MessageDto {
  uuid: string;
  direction: 'in' | 'out';
  content: string;
  sent_at: number;
  received_at: number | null;
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
