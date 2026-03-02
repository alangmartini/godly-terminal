import { registerSettingsTab } from './registry';
import { ThemesTab } from './themes-tab';
import { TerminalTab } from './terminal-tab';
import { NotificationsTab } from './notifications-tab';
import { PluginsTab } from './plugins-tab';
import { FlowsTab } from './flows-tab';
import { ShortcutsTab } from './shortcuts-tab';
import { RemoteTab } from './remote-tab';
import { AiToolsTab } from './ai-tools-tab';
import { QuickClaudeTab } from './quick-claude-tab';

// Register in display order
registerSettingsTab(new ThemesTab());
registerSettingsTab(new TerminalTab());
registerSettingsTab(new AiToolsTab());
registerSettingsTab(new QuickClaudeTab());
registerSettingsTab(new NotificationsTab());
registerSettingsTab(new PluginsTab());
registerSettingsTab(new FlowsTab());
registerSettingsTab(new ShortcutsTab());
registerSettingsTab(new RemoteTab());

export { getSettingsTabRegistry } from './registry';
