import { describe, expect, it } from 'vitest';
import { isAllowedExternalUrl } from './url';

describe('isAllowedExternalUrl', () => {
  it('allows https URLs', () => {
    expect(isAllowedExternalUrl('https://example.com')).toBe(true);
  });

  it('allows http URLs', () => {
    expect(isAllowedExternalUrl('http://example.com/path?q=1')).toBe(true);
  });

  it('blocks non-http schemes', () => {
    expect(isAllowedExternalUrl('javascript:alert(1)')).toBe(false);
    expect(isAllowedExternalUrl('file:///tmp/secrets.txt')).toBe(false);
    expect(isAllowedExternalUrl('data:text/html,<h1>hello</h1>')).toBe(false);
    expect(isAllowedExternalUrl('mailto:test@example.com')).toBe(false);
  });

  it('blocks invalid URLs', () => {
    expect(isAllowedExternalUrl('not a url')).toBe(false);
    expect(isAllowedExternalUrl('')).toBe(false);
    expect(isAllowedExternalUrl('/relative/path')).toBe(false);
  });
});
