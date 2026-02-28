import type { SettingsTabProvider } from './types';

const registry: SettingsTabProvider[] = [];

export function registerSettingsTab(provider: SettingsTabProvider): void {
  registry.push(provider);
}

export function getSettingsTabRegistry(): SettingsTabProvider[] {
  return [...registry];
}
