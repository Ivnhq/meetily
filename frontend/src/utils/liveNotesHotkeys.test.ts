import { describe, expect, it } from 'vitest';
import { isLiveNotesHighlightHotkey } from './liveNotesHotkeys';

function keyEvent(overrides: Partial<KeyboardEvent>): KeyboardEvent {
  return {
    altKey: false,
    ctrlKey: false,
    key: '',
    metaKey: false,
    repeat: false,
    shiftKey: false,
    ...overrides,
  } as KeyboardEvent;
}

describe('isLiveNotesHighlightHotkey', () => {
  it('accepts ctrl shift h', () => {
    expect(isLiveNotesHighlightHotkey(keyEvent({ ctrlKey: true, shiftKey: true, key: 'h' }))).toBe(true);
  });

  it('accepts meta shift h', () => {
    expect(isLiveNotesHighlightHotkey(keyEvent({ metaKey: true, shiftKey: true, key: 'H' }))).toBe(true);
  });

  it('ignores repeats and partial combinations', () => {
    expect(isLiveNotesHighlightHotkey(keyEvent({ ctrlKey: true, shiftKey: true, key: 'h', repeat: true }))).toBe(false);
    expect(isLiveNotesHighlightHotkey(keyEvent({ ctrlKey: true, key: 'h' }))).toBe(false);
    expect(isLiveNotesHighlightHotkey(keyEvent({ ctrlKey: true, shiftKey: true, altKey: true, key: 'h' }))).toBe(false);
  });
});
