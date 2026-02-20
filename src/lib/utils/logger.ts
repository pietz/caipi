import { error, warn, info, debug, trace, attachConsole } from '@tauri-apps/plugin-log';

export { error, warn, info, debug, trace };

export async function initLogger() {
	await attachConsole();
}
