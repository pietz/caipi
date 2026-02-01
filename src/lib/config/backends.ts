// Backend-specific configuration for models and thinking options

import type { Backend } from '$lib/stores/app.svelte';

export type ThinkingOption = { value: string; label: string };
export type ModelOption = { id: string; label: string; rings: 1 | 2 | 3 };

export interface BackendConfig {
  models: ModelOption[];
  defaultModel: string;
  thinkingOptions: ThinkingOption[];
  defaultThinking: string;
  hasThinkingOff: boolean; // Claude: true, Codex: false
  contextLimit: number; // Maximum context window in tokens
}

export const backendConfigs: Record<Backend, BackendConfig> = {
  claude: {
    models: [
      { id: 'opus', label: 'Opus 4.5', rings: 3 },
      { id: 'sonnet', label: 'Sonnet 4.5', rings: 2 },
      { id: 'haiku', label: 'Haiku 4.5', rings: 1 },
    ],
    defaultModel: 'sonnet',
    thinkingOptions: [
      { value: 'off', label: 'Off' },
      { value: 'on', label: 'On' },
    ],
    defaultThinking: 'on',
    hasThinkingOff: true,
    contextLimit: 200_000, // Claude models have 200k context
  },
  codex: {
    models: [
      { id: 'gpt-5.2', label: 'GPT-5.2', rings: 3 },
      { id: 'gpt-5.2-codex', label: 'GPT-5.2 Codex', rings: 2 },
      { id: 'gpt-5.1-codex-mini', label: 'GPT-5.1 Mini', rings: 1 },
    ],
    defaultModel: 'gpt-5.2',
    thinkingOptions: [
      { value: 'low', label: 'Low' },
      { value: 'medium', label: 'Medium' },
      { value: 'high', label: 'High' },
    ],
    defaultThinking: 'high',
    hasThinkingOff: false,
    contextLimit: 272_000, // Codex CLI reports ~272k usable context
  },
};
