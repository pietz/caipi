const ALLOWED_EXTERNAL_URL_PROTOCOLS = new Set(['http:', 'https:']);

export function isAllowedExternalUrl(value: string): boolean {
  try {
    const parsed = new URL(value);
    return ALLOWED_EXTERNAL_URL_PROTOCOLS.has(parsed.protocol.toLowerCase());
  } catch {
    return false;
  }
}
