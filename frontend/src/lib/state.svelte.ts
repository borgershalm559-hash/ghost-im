import type { ClientInfoDto, ContactDto, MessageDto } from './types';

class AppStore {
  info = $state<ClientInfoDto | null>(null);
  contacts = $state<ContactDto[]>([]);
  // contact ghost_id → message list. Mutated reactively when new messages arrive.
  threads = $state<Record<string, MessageDto[]>>({});

  setInfo(info: ClientInfoDto | null) {
    this.info = info;
  }

  setContacts(list: ContactDto[]) {
    this.contacts = list;
  }

  setThread(ghost_id: string, msgs: MessageDto[]) {
    this.threads = { ...this.threads, [ghost_id]: msgs };
  }

  pushIncoming(ghost_id: string, msg: MessageDto) {
    const existing = this.threads[ghost_id] ?? [];
    this.threads = { ...this.threads, [ghost_id]: [...existing, msg] };
  }
}

export const store = new AppStore();
