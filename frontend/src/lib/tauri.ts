import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ClientInfoDto,
  ContactDto,
  CreatedIdentityDto,
  IdentityStatusDto,
  InboundMessageEvent,
  InviteDto,
  MessageDto
} from './types';

export const INBOX_EVENT = 'ghost://message-received';

export async function identityStatus(): Promise<IdentityStatusDto> {
  return invoke('identity_status');
}

export async function createIdentity(
  display_name: string | null,
  passphrase: string | null
): Promise<CreatedIdentityDto> {
  return invoke('create_identity', { displayName: display_name, passphrase });
}

export async function openClient(passphrase: string | null): Promise<ClientInfoDto> {
  return invoke('open_client', { passphrase });
}

export async function clientInfo(): Promise<ClientInfoDto> {
  return invoke('client_info');
}

export async function listContacts(): Promise<ContactDto[]> {
  return invoke('list_contacts');
}

export async function listMessages(
  contact_ghost_id: string,
  limit = 200,
  offset = 0
): Promise<MessageDto[]> {
  return invoke('list_messages', { contactGhostId: contact_ghost_id, limit, offset });
}

export async function createInvite(ttl_seconds = 7 * 24 * 3600): Promise<InviteDto> {
  return invoke('create_invite', { ttlSeconds: ttl_seconds });
}

export async function addContact(invite: string): Promise<void> {
  return invoke('add_contact', { invite });
}

export async function sendMessage(contact_ghost_id: string, text: string): Promise<void> {
  return invoke('send_message', { contactGhostId: contact_ghost_id, text });
}

export async function onInbound(
  cb: (e: InboundMessageEvent) => void
): Promise<UnlistenFn> {
  return listen<InboundMessageEvent>(INBOX_EVENT, (event) => cb(event.payload));
}
