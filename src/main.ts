import { App } from './components/App';
import { store } from './state/store';
import { notificationStore } from './state/notification-store';
import { initLogger } from './utils/Logger';
import { initPlugins } from './plugins/index';
import { initFlowEngine } from './flow-engine/index';

initLogger();

// Expose store globally for MCP execute_js tool
(window as any).__STORE__ = store;
(window as any).__NOTIFICATION_STORE__ = notificationStore;

// Prevent WebView2 native zoom on Ctrl+scroll/keyboard everywhere in the app.
// The terminal canvas has its own Ctrl+scroll handler for font-size zoom, but
// events on other elements (tab bar, sidebar) would otherwise trigger native
// browser zoom, causing content to not fill the window (black border).
document.addEventListener('wheel', (e) => {
  if (e.ctrlKey) e.preventDefault();
}, { passive: false });

document.addEventListener('keydown', (e) => {
  if (e.ctrlKey && !e.shiftKey && !e.altKey &&
      (e.key === '+' || e.key === '-' || e.key === '=' || e.key === '0')) {
    e.preventDefault();
  }
});

const container = document.getElementById('app');
if (!container) {
  throw new Error('App container not found');
}

const app = new App(container);
app.init().then(async () => {
  await initPlugins();
  initFlowEngine();
}).catch(console.error);
