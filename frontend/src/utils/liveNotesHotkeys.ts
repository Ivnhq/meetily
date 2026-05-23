export function isLiveNotesHighlightHotkey(
  event: Pick<KeyboardEvent, 'altKey' | 'ctrlKey' | 'key' | 'metaKey' | 'repeat' | 'shiftKey'>
): boolean {
  return (
    !event.repeat &&
    !event.altKey &&
    event.shiftKey &&
    (event.ctrlKey || event.metaKey) &&
    event.key.toLowerCase() === 'h'
  );
}

export function isEditableHotkeyTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;

  const tagName = target.tagName.toLowerCase();
  return target.isContentEditable || tagName === 'input' || tagName === 'textarea' || tagName === 'select';
}
