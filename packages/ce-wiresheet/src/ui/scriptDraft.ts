// In-memory unsaved-source cache keyed by scriptId. Because submenu switches
// remount the editors, an edited-but-unsaved script would otherwise lose its
// changes; the editors stash the draft here and restore it on mount. Saving (or
// an explicit Reload) clears the entry. Shared across the Components and Scripts
// submenus, so editing "the same script" in either is the same draft.
const drafts = new Map<string, string>();

export const getDraft = (id: string): string | undefined => drafts.get(id);
export const hasDraft = (id: string): boolean => drafts.has(id);
export const setDraft = (id: string, src: string): void => { drafts.set(id, src); };
export const clearDraft = (id: string): void => { drafts.delete(id); };
