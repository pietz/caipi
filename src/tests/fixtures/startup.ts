import type { CliStatus, StartupInfo } from '$lib/api/types';

export const defaultCliStatus: CliStatus = {
  installed: true,
  authenticated: true,
  version: '1.0.0',
  path: '/usr/local/bin/claude',
};

export const defaultStartupInfo: StartupInfo = {
  onboardingCompleted: false,
  cliStatusFresh: true,
};

export function makeStartupInfo(overrides: Partial<StartupInfo> = {}): StartupInfo {
  return {
    ...defaultStartupInfo,
    ...overrides,
  };
}
