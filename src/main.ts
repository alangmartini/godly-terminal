import '@xterm/xterm/css/xterm.css';
import { App } from './components/App';

const container = document.getElementById('app');
if (!container) {
  throw new Error('App container not found');
}

const app = new App(container);
app.init().catch(console.error);
