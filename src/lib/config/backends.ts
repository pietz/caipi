// Backend configuration for different AI coding CLIs

export type Backend = 'claude' | 'claudecli';

export interface ThinkingOption {
  value: string;
  label: string;
}

export interface ModelConfig {
  id: string;
  name: string;
  size: 'large' | 'medium' | 'small';
  supportsThinking: boolean;
}

export interface BackendConfig {
  models: ModelConfig[];
  thinkingOptions: ThinkingOption[];
  defaultThinking: string;
  contextLimit: number;
}

export const backendConfigs: Record<Backend, BackendConfig> = {
  claude: {
    models: [
      { id: 'opus', name: 'Opus 4.6', size: 'large', supportsThinking: true },
      { id: 'sonnet', name: 'Sonnet 4.5', size: 'medium', supportsThinking: true },
      { id: 'haiku', name: 'Haiku 4.5', size: 'small', supportsThinking: false },
    ],
    thinkingOptions: [
      { value: 'off', label: 'Off' },
      { value: 'on', label: 'On' },
    ],
    defaultThinking: 'on',
    contextLimit: 200_000,
  },
  claudecli: {
    models: [
      { id: 'opus', name: 'Opus 4.6', size: 'large', supportsThinking: true },
      { id: 'sonnet', name: 'Sonnet 4.5', size: 'medium', supportsThinking: true },
      { id: 'haiku', name: 'Haiku 4.5', size: 'small', supportsThinking: false },
    ],
    thinkingOptions: [
      { value: 'off', label: 'Off' },
      { value: 'on', label: 'On' },
    ],
    defaultThinking: 'on',
    contextLimit: 200_000,
  },
};

// Get config for the current backend
export function getBackendConfig(backend: Backend = 'claudecli'): BackendConfig {
  return backendConfigs[backend];
}
