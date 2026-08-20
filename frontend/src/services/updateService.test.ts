import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const updaterMocks = vi.hoisted(() => ({
  check: vi.fn(),
  getVersion: vi.fn(),
  relaunch: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: updaterMocks.check,
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: updaterMocks.relaunch,
}));

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: updaterMocks.getVersion,
}));

describe('UpdateService local distribution guard', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    updaterMocks.getVersion.mockResolvedValue('0.3.0');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('does not query the upstream updater in an Ivnhq local build', async () => {
    vi.stubEnv('NEXT_PUBLIC_MEETILY_DISABLE_UPDATES', '1');
    const { UpdateService } = await import('./updateService');

    const result = await new UpdateService().checkForUpdates(true);

    expect(result).toEqual({
      available: false,
      currentVersion: '0.3.0',
      disabled: true,
    });
    expect(updaterMocks.check).not.toHaveBeenCalled();
  });

  it('refuses installation in an Ivnhq local build even with an update object', async () => {
    vi.stubEnv('NEXT_PUBLIC_MEETILY_DISABLE_UPDATES', '1');
    const { UpdateService } = await import('./updateService');
    const update = {
      download: vi.fn(),
      install: vi.fn(),
    };

    await expect(
      new UpdateService().downloadAndInstall(update as never)
    ).rejects.toThrow('Updates are disabled for this custom build');
    expect(update.download).not.toHaveBeenCalled();
    expect(update.install).not.toHaveBeenCalled();
    expect(updaterMocks.relaunch).not.toHaveBeenCalled();
  });
});
