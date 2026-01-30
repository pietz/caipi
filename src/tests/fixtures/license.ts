import type { LicenseStatus } from '$lib/api/types';

export const validLicense: LicenseStatus = {
  valid: true,
  licenseKey: 'test-license',
  activatedAt: 1700000000,
  email: 'test@example.com',
};

export const invalidLicense: LicenseStatus = {
  valid: false,
};
