import { App } from './components/App';
import { initLogger } from './utils/Logger';

initLogger();

const container = document.getElementById('app');
if (!container) {
  throw new Error('App container not found');
}

const app = new App(container);
app.init().catch(console.error);
