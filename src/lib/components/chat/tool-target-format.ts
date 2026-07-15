import { summarizeThinking } from '$lib/utils/thinking';

export function getCompactToolTarget(toolType: string, target: string): string {
  if (toolType !== 'Thinking') {
    return target;
  }

  return summarizeThinking(target);
}
