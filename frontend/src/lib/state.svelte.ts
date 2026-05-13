import type { ClientInfoDto, ContactDto, MessageDto } from './types';
import type { Theme } from './theme';

class AppStore {
  info = $state<ClientInfoDto | null>(null);
  contacts = $state<ContactDto[]>([]);
  threads = $state<Record<string, MessageDto[]>>({});

  /** Currently visible theme; bootTheme() sets it on app start. */
  theme = $state<Theme>('dark');
  /** Persisted ghost-mode flag (visual-only in MVP-1). */
  ghostMode = $state(false);

  /** Sidebar search filter (local; not persisted). */
  searchQuery = $state('');

  /** Currently selected folder in the left rail. 'all' is the only one
   * with real contact data wired through; the rest are placeholders that
   * show a "coming soon" message in the chat list. */
  activeFolder = $state('all');

  setInfo(info: ClientInfoDto | null) {
    this.info = info;
  }

  setContacts(list: ContactDto[]) {
    this.contacts = list;
  }

  /** Replace a single contact's row (after pin/mute/verify/retention edits). */
  patchContact(ghost_id: string, patch: Partial<ContactDto>) {
    this.contacts = this.contacts.map((c) =>
      c.ghost_id === ghost_id ? { ...c, ...patch } : c
    );
  }

  setThread(ghost_id: string, msgs: MessageDto[]) {
    this.threads = { ...this.threads, [ghost_id]: msgs };
  }

  pushIncoming(ghost_id: string, msg: MessageDto) {
    const existing = this.threads[ghost_id] ?? [];
    this.threads = { ...this.threads, [ghost_id]: [...existing, msg] };
  }

  setTheme(t: Theme) {
    this.theme = t;
  }

  setGhostMode(on: boolean) {
    this.ghostMode = on;
  }

  setSearchQuery(q: string) {
    this.searchQuery = q;
  }

  setActiveFolder(id: string) {
    this.activeFolder = id;
  }
}

export const store = new AppStore();
