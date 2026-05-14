import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ClientInfoDto,
  ContactDto,
  CreatedIdentityDto,
  IdentityStatusDto,
  InboundMessageEvent,
  InviteDto,
  MessageDto,
  UpdateAvailableDto
} from './types';

export type { UpdateAvailableDto };

export const INBOX_EVENT = 'ghost://message-received';
export const UPDATE_PROGRESS_EVENT = 'ghost://update-progress';

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

export async function checkForUpdate(): Promise<UpdateAvailableDto | null> {
  return invoke('check_for_update');
}

export async function downloadAndInstallUpdate(): Promise<void> {
  return invoke('download_and_install_update');
}

export interface UpdateProgress {
  chunk: number;
  total: number;
}

export async function onUpdateProgress(
  cb: (p: UpdateProgress) => void
): Promise<UnlistenFn> {
  return listen<UpdateProgress>(UPDATE_PROGRESS_EVENT, (event) => cb(event.payload));
}

// ─── Per-contact actions ────────────────────────────────────────────────────

export async function setPinned(contact_ghost_id: string, pinned: boolean): Promise<void> {
  return invoke('set_pinned', { contactGhostId: contact_ghost_id, pinned });
}

export async function setMuted(contact_ghost_id: string, muted: boolean): Promise<void> {
  return invoke('set_muted', { contactGhostId: contact_ghost_id, muted });
}

export async function setVerified(contact_ghost_id: string, verified: boolean): Promise<void> {
  return invoke('set_verified', { contactGhostId: contact_ghost_id, verified });
}

export async function setRetention(
  contact_ghost_id: string,
  seconds: number | null
): Promise<void> {
  return invoke('set_retention', { contactGhostId: contact_ghost_id, seconds });
}

export async function markChatRead(contact_ghost_id: string): Promise<void> {
  return invoke('mark_chat_read', { contactGhostId: contact_ghost_id });
}

// ─── Settings ───────────────────────────────────────────────────────────────

export async function getSetting(key: string): Promise<string | null> {
  return invoke('get_setting', { key });
}

export async function setSetting(key: string, value: string): Promise<void> {
  return invoke('set_setting', { key, value });
}

// ─── Backup / Restore ──────────────────────────────────────────────────────

export async function exportBackup(path: string, passphrase: string): Promise<number> {
  return invoke('export_backup', { path, passphrase });
}

export async function importBackup(path: string, passphrase: string): Promise<void> {
  return invoke('import_backup', { path, passphrase });
}

// ─── Folders ───────────────────────────────────────────────────────────────

export interface FolderDto {
  id: number;
  name: string;
  icon: string | null;
  sort_order: number;
  created_at: number;
}

export async function listFolders(): Promise<FolderDto[]> {
  return invoke('list_folders');
}

export async function createFolder(name: string, icon: string | null = null): Promise<number> {
  return invoke('create_folder', { name, icon });
}

export async function renameFolder(folder_id: number, new_name: string): Promise<void> {
  return invoke('rename_folder', { folderId: folder_id, newName: new_name });
}

export async function deleteFolder(folder_id: number): Promise<boolean> {
  return invoke('delete_folder', { folderId: folder_id });
}

export async function addContactToFolder(folder_id: number, contact_ghost_id: string): Promise<void> {
  return invoke('add_contact_to_folder', { folderId: folder_id, contactGhostId: contact_ghost_id });
}

export async function removeContactFromFolder(folder_id: number, contact_ghost_id: string): Promise<void> {
  return invoke('remove_contact_from_folder', { folderId: folder_id, contactGhostId: contact_ghost_id });
}

export async function contactsForFolder(folder_id: number): Promise<string[]> {
  return invoke('contacts_for_folder', { folderId: folder_id });
}

// ─── Diagnostics ───────────────────────────────────────────────────────────

export interface DiagnosticsDto {
  app_version: string;
  ghost_id: string;
  fingerprint: string;
  peer_id: string;
  local_addrs: string[];
  bootstrap_count: number;
}

export async function getDiagnostics(): Promise<DiagnosticsDto> {
  return invoke('get_diagnostics');
}
