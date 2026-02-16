import { describe, expect, it } from 'vitest';
import { getToolConfig } from './tool-configs';

describe('tool-configs', () => {
  it('maps codex spawn_agent and wait tool labels', () => {
    expect(getToolConfig('spawn_agent').label).toBe('agent');
    expect(getToolConfig('wait').label).toBe('wait');
  });

  it('maps web tool variants to expected labels', () => {
    expect(getToolConfig('web_search').label).toBe('search');
    expect(getToolConfig('web_fetch').label).toBe('fetch');
  });
});
