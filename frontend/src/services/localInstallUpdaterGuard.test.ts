import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('macOS local installer updater guard', () => {
  const installer = readFileSync(
    resolve(process.cwd(), '../scripts/install-macos-live-notes.sh'),
    'utf8'
  );

  it('builds the Ivnhq app with frontend update checks disabled', () => {
    expect(installer).toContain('NEXT_PUBLIC_MEETILY_DISABLE_UPDATES=1');
  });

  it('does not leave the official Meetily release feed attached to the custom app', () => {
    expect(installer).toContain(
      'https://github.com/Ivnhq/meetily/releases/latest/download/latest.json'
    );
  });

  it('does not upgrade the entire build toolchain on every repair', () => {
    expect(installer).toContain('MISSING_FORMULAE=()');
    expect(installer).not.toContain('brew install git node rustup cmake ollama');
  });

  it('checks that a build root exists before searching it', () => {
    expect(installer).toContain('if [[ -d "$search_root" ]]');
    expect(installer).not.toContain('find target frontend/src-tauri/target');
  });
});
