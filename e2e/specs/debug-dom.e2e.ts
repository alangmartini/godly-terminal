/**
 * Diagnostic test - check if the flush fix resolves the daemon communication issue.
 */

describe('Debug DOM State', () => {
  it('should observe terminal creation', async () => {
    console.log('[test] Waiting 15s for app initialization...');
    await browser.pause(15000);

    // Check DOM state
    const state = await browser.execute(() => {
      const selectors = [
        '#app', '.sidebar', '.main-content', '.tab-bar',
        '.terminal-container', '.terminal-pane', '.terminal-pane.active',
        '.empty-state', '.tab', '.add-tab-btn', '.workspace-item',
      ];
      const result: Record<string, number> = {};
      for (const sel of selectors) {
        result[sel] = document.querySelectorAll(sel).length;
      }
      return result;
    });
    console.log('[test] DOM state:', JSON.stringify(state));

    // Check body HTML if empty
    if (state['#app'] === 0) {
      const bodyInfo = await browser.execute(() => {
        return {
          url: window.location.href,
          bodyHTML: document.body?.innerHTML?.substring(0, 500) || 'NO BODY',
          bodyChildren: document.body?.children?.length || 0,
        };
      });
      console.log('[test] Body info:', JSON.stringify(bodyInfo));
    }

    // Check init errors
    const errors = await browser.execute(() => ({
      initError: (window as any).__app_init_error || null,
      initError2: (window as any).__app_init_error2 || null,
    }));
    console.log('[test] Init errors:', JSON.stringify(errors));

    // If terminal pane exists, try reading it
    if (state['.terminal-pane.active'] > 0) {
      console.log('[test] SUCCESS - Terminal pane is active!');
    } else {
      console.log('[test] FAIL - No active terminal pane found');
    }
  });
});
