/**
 * SVG path data for icons used across the sidebar redesign.
 * Each icon is a `d` string plus default stroke-width. Render with the
 * Icon.svelte component.
 *
 * Stroke-based, currentColor, viewBox 0 0 24 24.
 */

export interface IconDef {
  /** SVG path data; may contain multiple subpaths separated by `M`. */
  d: string;
  /** Default stroke-width (caller can override). */
  sw: number;
  /** If `true`, render with fill=currentColor instead of stroke. */
  fill?: boolean;
}

export const I: Record<string, IconDef> = {
  search: { d: 'M11 19a8 8 0 1 1 0-16 8 8 0 0 1 0 16ZM21 21l-4.3-4.3', sw: 1.6 },
  plus: { d: 'M12 5v14M5 12h14', sw: 1.6 },
  settings: {
    d: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h.1a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z',
    sw: 1.6,
  },
  pin: { d: 'M12 17v5 M9 3h6l-1 5 3 3v2H7v-2l3-3-1-5Z', sw: 1.6 },
  archive: { d: 'M3 7h18 M5 7v12a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7 M9 12h6 M3 4h18v3H3z', sw: 1.6 },
  shield: { d: 'M12 3 4 6v6c0 5 3.5 8 8 9 4.5-1 8-4 8-9V6l-8-3Z', sw: 1.6 },
  lock: { d: 'M5 11h14v10H5z M8 11V8a4 4 0 0 1 8 0v3', sw: 1.6 },
  send: { d: 'M22 2 11 13 M22 2l-7 20-4-9-9-4 20-7Z', sw: 1.6 },
  hash: { d: 'M4 9h16 M4 15h16 M10 3 8 21 M16 3l-2 18', sw: 1.6 },
  bellOff: { d: 'M13.7 21a2 2 0 0 1-3.4 0 M18 8a6 6 0 0 0-9.3-5 M6 8c0 7-3 9-3 9h12 M2 2l20 20', sw: 1.6 },
  ghost: { d: 'M12 2a8 8 0 0 0-8 8v11l3-2 3 2 2-2 2 2 3-2 3 2V10a8 8 0 0 0-8-8Z M9 11h.01 M15 11h.01', sw: 1.6 },
  checkDouble: { d: 'M2 12l5 5L18 6 M9 17l1.5 1.5L22 7', sw: 1.6 },
  fire: { d: 'M14 3c0 4-4 5-4 9a4 4 0 0 0 8 0c0-2-1-3-2-4 0 0 1 5-2 5 0-3-3-4 0-10Z M6 14a4 4 0 0 0 4 6', sw: 1.6 },
  user: { d: 'M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2 M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z', sw: 1.6 },
  users: { d: 'M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2 M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z M23 21v-2a4 4 0 0 0-3-3.9 M16 3.1a4 4 0 0 1 0 7.8', sw: 1.6 },
  inbox: { d: 'M22 12h-6l-2 3h-4l-2-3H2 M5.5 5h13L22 12v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-6L5.5 5Z', sw: 1.6 },
  key: { d: 'm21 2-2 2m-7.6 7.6a5.5 5.5 0 1 1-7.8 7.8 5.5 5.5 0 0 1 7.8-7.8Zm0 0L15 8m0 0 4 4m-4-4 3-3', sw: 1.6 },
  more: { d: 'M12 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z M19 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z M5 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z', sw: 1.6, fill: true },
  edit: { d: 'M12 20h9 M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z', sw: 1.6 },
};

export type IconName = keyof typeof I;
