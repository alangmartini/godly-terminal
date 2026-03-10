import { WebTestAdapter } from './web-adapter';

export class TestHarnessBridge {
  private adapter: WebTestAdapter;

  constructor() {
    this.adapter = new WebTestAdapter();
  }

  init() {
    // Register on window for backend access via execute_js
    (window as any).__TEST_HARNESS__ = {
      query: (q: any) => this.adapter.query(q),
      act: (a: any) => this.adapter.act(a),
      wait: (w: any) => this.adapter.wait(w),
      isReady: () => true,
    };

    console.log('[test-harness] Web adapter bridge initialized');
  }

  destroy() {
    delete (window as any).__TEST_HARNESS__;
  }
}
