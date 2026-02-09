// Backend configuration for different AI coding CLIs

export type Backend = 'claude' | 'claudecli' | 'codex';

export interface ThinkingOption {
  value: string;
  label: string;
}

export interface ModelConfig {
  id: string;
  name: string;
  size: 'large' | 'medium' | 'small';
  thinkingOptions: ThinkingOption[];
  defaultThinking: string;
}

export interface BackendConfig {
  models: ModelConfig[];
  contextLimit: number;
}

export const backendConfigs: Record<Backend, BackendConfig> = {
  claude: {
    models: [
      { id: 'opus', name: 'Opus 4.6', size: 'large', thinkingOptions: [
        { value: 'low', label: 'Low' },
        { value: 'medium', label: 'Med' },
        { value: 'high', label: 'High' },
        { value: 'max', label: 'Max' },
      ], defaultThinking: 'high' },
      { id: 'sonnet', name: 'Sonnet 4.5', size: 'medium', thinkingOptions: [
        { value: 'off', label: 'Off' },
        { value: 'on', label: 'On' },
      ], defaultThinking: 'on' },
      { id: 'haiku', name: 'Haiku 4.5', size: 'small', thinkingOptions: [], defaultThinking: '' },
    ],
    contextLimit: 200_000,
  },
  claudecli: {
    models: [
      { id: 'opus', name: 'Opus 4.6', size: 'large', thinkingOptions: [], defaultThinking: '' },
      { id: 'sonnet', name: 'Sonnet 4.5', size: 'medium', thinkingOptions: [], defaultThinking: '' },
      { id: 'haiku', name: 'Haiku 4.5', size: 'small', thinkingOptions: [], defaultThinking: '' },
    ],
    contextLimit: 200_000,
  },
  codex: {
    models: [
      { id: 'gpt-5.3-codex', name: 'GPT-5.3 Codex', size: 'large', thinkingOptions: [
        { value: 'low', label: 'Low' },
        { value: 'medium', label: 'Med' },
        { value: 'high', label: 'High' },
      ], defaultThinking: 'medium' },
      { id: 'gpt-5.2', name: 'GPT-5.2', size: 'medium', thinkingOptions: [
        { value: 'low', label: 'Low' },
        { value: 'medium', label: 'Med' },
        { value: 'high', label: 'High' },
      ], defaultThinking: 'medium' },
      { id: 'gpt-5.1-codex-mini', name: 'GPT-5.1 Codex Mini', size: 'small', thinkingOptions: [], defaultThinking: '' },
    ],
    contextLimit: 258_000,
  },
};

// Get config for the current backend
export function getBackendConfig(backend: Backend = 'claudecli'): BackendConfig {
  return backendConfigs[backend];
}
