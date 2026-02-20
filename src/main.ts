import { App } from './components/App';
import { initLogger } from './utils/Logger';
import { initPlugins } from './plugins/index';

initLogger();

const container = document.getElementById('app');
if (!container) {
  throw new Error('App container not found');
}

const app = new App(container);
app.init().then(() => {
  initPlugins();
}).catch(console.error);
