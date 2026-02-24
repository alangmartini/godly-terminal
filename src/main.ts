import { App } from './components/App';
import { store } from './state/store';
import { initLogger } from './utils/Logger';
import { initPlugins } from './plugins/index';

initLogger();

// Expose store globally for MCP execute_js tool
(window as any).__STORE__ = store;

const container = document.getElementById('app');
if (!container) {
  throw new Error('App container not found');
}

const app = new App(container);
app.init().then(async () => {
  await initPlugins();
}).catch(console.error);
