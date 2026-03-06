#!/usr/bin/env node
/**
 * Phone UI QA — Playwright mobile emulation
 *
 * Runs against http://localhost:3377/phone with iPhone emulation.
 * Takes screenshots at each step and logs issues found.
 */
import { chromium, devices } from 'playwright';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

const BASE = 'http://localhost:3377';
const API_KEY = 'testkey123';
const PHONE_URL = `${BASE}/phone#key=${API_KEY}`;
const SCREENSHOT_DIR = join(import.meta.dirname, '..', 'qa-screenshots');
mkdirSync(SCREENSHOT_DIR, { recursive: true });

const issues = [];
let screenshotIdx = 0;

async function screenshot(page, name) {
  const idx = String(++screenshotIdx).padStart(2, '0');
  const path = join(SCREENSHOT_DIR, `${idx}-${name}.png`);
  await page.screenshot({ path, fullPage: false });
  console.log(`  Screenshot: ${idx}-${name}.png`);
  return path;
}

function logIssue(severity, title, detail) {
  issues.push({ severity, title, detail });
  console.log(`  [${severity.toUpperCase()}] ${title}: ${detail}`);
}

async function main() {
  console.log('=== Godly Mobile QA ===\n');

  // Check health first
  try {
    const resp = await fetch(`${BASE}/health`);
    if (!resp.ok) throw new Error(`Health check failed: ${resp.status}`);
    console.log('Server healthy.\n');
  } catch (e) {
    console.error(`Cannot reach server at ${BASE}: ${e.message}`);
    process.exit(1);
  }

  const browser = await chromium.launch({ headless: false });

  // Test on multiple device profiles
  const deviceProfiles = [
    { name: 'iPhone 14', device: devices['iPhone 14'] },
    { name: 'Pixel 7', device: devices['Pixel 7'] },
  ];

  for (const { name: deviceName, device } of deviceProfiles) {
    console.log(`\n--- Testing on ${deviceName} ---\n`);

    const context = await browser.newContext({
      ...device,
      baseURL: BASE,
    });
    const page = await context.newPage();

    // Collect console errors
    const consoleErrors = [];
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });

    // ===== 1. Load phone page =====
    console.log('1. Loading phone page...');
    await page.goto(PHONE_URL, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(3000); // Let init() + SSE connect
    await screenshot(page, `${deviceName}-01-initial-load`);

    // Check which view is active
    const activeView = await page.evaluate(() => {
      const views = document.querySelectorAll('.view.active');
      return views.length > 0 ? views[0].id : 'none';
    });
    console.log(`  Active view: ${activeView}`);

    // If login is shown, that's expected with password auth
    // Since we set API key via fragment, check if we got to dashboard
    if (activeView === 'view-login') {
      console.log('  Login view shown - checking if password is needed...');
      await screenshot(page, `${deviceName}-01b-login-view`);

      // Try registering without password (no password was set on server)
      const hasPasswordField = await page.locator('#loginPassword').isVisible();
      if (hasPasswordField) {
        // No password was configured on this server, so registration might have auto-succeeded
        // Wait a bit more
        await page.waitForTimeout(1000);
        const viewAfterWait = await page.evaluate(() => {
          const views = document.querySelectorAll('.view.active');
          return views.length > 0 ? views[0].id : 'none';
        });
        if (viewAfterWait === 'view-login') {
          logIssue('info', 'Login required', 'Server requires password - will try empty submit');
        }
      }
    }

    // ===== 2. Test Dashboard =====
    if (activeView === 'view-dashboard' || activeView !== 'view-login') {
      console.log('\n2. Dashboard view...');
      await screenshot(page, `${deviceName}-02-dashboard`);

      // Check for workspaces
      const workspaceCount = await page.locator('.ws-card').count();
      console.log(`  Workspaces found: ${workspaceCount}`);

      // Check for terminal rows
      const terminalCount = await page.locator('.term-row').count();
      console.log(`  Terminal rows found: ${terminalCount}`);

      // Test dashboard scrolling
      console.log('\n3. Testing dashboard scroll behavior...');
      const dashboardViewport = await page.evaluate(() => {
        return {
          scrollHeight: document.body.scrollHeight,
          clientHeight: document.body.clientHeight,
          scrollable: document.body.scrollHeight > document.body.clientHeight,
        };
      });
      console.log(`  Body scrollHeight: ${dashboardViewport.scrollHeight}, clientHeight: ${dashboardViewport.clientHeight}`);
      console.log(`  Dashboard scrollable: ${dashboardViewport.scrollable}`);

      // ===== 3. Open a terminal session =====
      if (terminalCount > 0) {
        console.log('\n4. Opening first terminal session...');
        await page.locator('.term-row').first().click();
        await page.waitForTimeout(2000);
        await screenshot(page, `${deviceName}-04-session-view`);

        // Check session output
        const outputInfo = await page.evaluate(() => {
          const output = document.getElementById('sessionOutput');
          if (!output) return null;
          const rect = output.getBoundingClientRect();
          return {
            text: output.textContent?.slice(0, 100),
            scrollHeight: output.scrollHeight,
            clientHeight: output.clientHeight,
            scrollTop: output.scrollTop,
            isScrollable: output.scrollHeight > output.clientHeight,
            rect: { top: rect.top, bottom: rect.bottom, height: rect.height },
            maxHeight: getComputedStyle(output).maxHeight,
            overflow: getComputedStyle(output).overflow,
          };
        });
        console.log(`  Session output info:`, JSON.stringify(outputInfo, null, 2));

        // ===== 4. SCROLL TESTING (main reported bug) =====
        console.log('\n5. Testing session output scroll behavior...');

        // a) Check if page itself scrolls when trying to scroll session output
        const scrollTestResult = await page.evaluate(() => {
          const results = [];
          const output = document.getElementById('sessionOutput');
          const body = document.body;

          // Record initial positions
          const initialBodyScroll = window.scrollY;
          const initialOutputScroll = output ? output.scrollTop : 0;

          results.push({
            test: 'initial-state',
            bodyScroll: initialBodyScroll,
            outputScroll: initialOutputScroll,
            bodyScrollable: body.scrollHeight > window.innerHeight,
            outputScrollable: output ? output.scrollHeight > output.clientHeight : false,
          });

          return results;
        });
        console.log(`  Scroll test:`, JSON.stringify(scrollTestResult, null, 2));

        // b) Test touch scrolling on the session output
        const outputBox = await page.locator('#sessionOutput').boundingBox();
        if (outputBox) {
          // Simulate touch scroll down on session output
          const centerX = outputBox.x + outputBox.width / 2;
          const centerY = outputBox.y + outputBox.height / 2;

          // Record scroll positions before touch
          const beforeTouch = await page.evaluate(() => ({
            bodyScroll: window.scrollY,
            outputScroll: document.getElementById('sessionOutput')?.scrollTop || 0,
          }));

          // Simulate a swipe up gesture (scroll down) on the output area
          await page.touchscreen.tap(centerX, centerY);
          await page.waitForTimeout(100);

          // Try to scroll by swiping
          // Start from bottom of output, swipe up
          const startY = outputBox.y + outputBox.height - 20;
          const endY = outputBox.y + 20;

          await page.evaluate(async ({ sx, sy, ex, ey }) => {
            // Dispatch touch events manually for better control
            const outputEl = document.getElementById('sessionOutput');
            const target = document.elementFromPoint(sx, sy) || outputEl;

            const touchStart = new TouchEvent('touchstart', {
              bubbles: true, cancelable: true,
              touches: [new Touch({ identifier: 1, target, clientX: sx, clientY: sy })],
              targetTouches: [new Touch({ identifier: 1, target, clientX: sx, clientY: sy })],
            });
            target.dispatchEvent(touchStart);

            // Multiple touchmove events to simulate drag
            for (let i = 0; i < 10; i++) {
              const progress = i / 10;
              const cy = sy + (ey - sy) * progress;
              await new Promise(r => setTimeout(r, 16));
              const touchMove = new TouchEvent('touchmove', {
                bubbles: true, cancelable: true,
                touches: [new Touch({ identifier: 1, target, clientX: sx, clientY: cy })],
                targetTouches: [new Touch({ identifier: 1, target, clientX: sx, clientY: cy })],
              });
              target.dispatchEvent(touchMove);
            }

            const touchEnd = new TouchEvent('touchend', {
              bubbles: true, cancelable: true,
              changedTouches: [new Touch({ identifier: 1, target, clientX: ex, clientY: ey })],
            });
            target.dispatchEvent(touchEnd);
          }, { sx: centerX, sy: startY, ex: centerX, ey: endY });

          await page.waitForTimeout(500);

          const afterTouch = await page.evaluate(() => ({
            bodyScroll: window.scrollY,
            outputScroll: document.getElementById('sessionOutput')?.scrollTop || 0,
          }));

          console.log(`  Before touch: body=${beforeTouch.bodyScroll}, output=${beforeTouch.outputScroll}`);
          console.log(`  After touch:  body=${afterTouch.bodyScroll}, output=${afterTouch.outputScroll}`);

          // Check if body scrolled when it shouldn't have
          if (afterTouch.bodyScroll !== beforeTouch.bodyScroll) {
            logIssue('critical', 'Body scrolls during session output interaction',
              `Body scrollY changed from ${beforeTouch.bodyScroll} to ${afterTouch.bodyScroll} while touching session output`);
          }

          await screenshot(page, `${deviceName}-05-after-scroll-test`);
        }

        // c) Test auto-scroll vs user scroll conflict (the "skip" bug)
        console.log('\n6. Testing auto-scroll refresh conflict...');

        // The refreshSession() runs every 1000ms and sets scrollTop = scrollHeight
        // This fights with user scroll attempts
        const refreshConflict = await page.evaluate(() => {
          const output = document.getElementById('sessionOutput');
          if (!output || output.scrollHeight <= output.clientHeight) {
            return { testable: false, reason: 'Output not scrollable' };
          }

          // Scroll to middle
          const midpoint = output.scrollHeight / 2;
          output.scrollTop = midpoint;
          const afterManualScroll = output.scrollTop;

          return {
            testable: true,
            scrollHeight: output.scrollHeight,
            clientHeight: output.clientHeight,
            manualScrollTo: midpoint,
            actualScrollAfterManual: afterManualScroll,
          };
        });
        console.log(`  Refresh conflict test:`, JSON.stringify(refreshConflict, null, 2));

        if (refreshConflict.testable) {
          // Wait for a refresh cycle (1000ms)
          await page.waitForTimeout(1500);

          const afterRefresh = await page.evaluate(() => {
            const output = document.getElementById('sessionOutput');
            return {
              scrollTop: output.scrollTop,
              scrollHeight: output.scrollHeight,
              isAtBottom: Math.abs(output.scrollTop + output.clientHeight - output.scrollHeight) < 5,
            };
          });
          console.log(`  After refresh cycle:`, JSON.stringify(afterRefresh, null, 2));

          if (afterRefresh.isAtBottom) {
            logIssue('critical', 'Auto-refresh overrides user scroll position',
              `User scrolled to middle, but after 1s refresh cycle, scroll jumped to bottom. ` +
              `This makes it impossible to read scrollback on mobile.`);
          }
        }

        await screenshot(page, `${deviceName}-06-after-refresh-test`);

        // ===== 5. Test input bar =====
        console.log('\n7. Testing input bar...');
        const inputBarVisible = await page.locator('#inputBar.active').isVisible();
        console.log(`  Input bar visible: ${inputBarVisible}`);

        if (inputBarVisible) {
          // Check if input bar covers content
          const overlap = await page.evaluate(() => {
            const inputBar = document.getElementById('inputBar');
            const output = document.getElementById('sessionOutput');
            if (!inputBar || !output) return null;

            const barRect = inputBar.getBoundingClientRect();
            const outRect = output.getBoundingClientRect();

            return {
              inputBarTop: barRect.top,
              inputBarHeight: barRect.height,
              outputBottom: outRect.bottom,
              windowHeight: window.innerHeight,
              overlap: outRect.bottom > barRect.top,
              gapBelowOutput: barRect.top - outRect.bottom,
            };
          });
          console.log(`  Layout overlap check:`, JSON.stringify(overlap, null, 2));

          if (overlap && overlap.overlap) {
            logIssue('major', 'Input bar overlaps session output',
              `Input bar at y=${overlap.inputBarTop} overlaps output bottom at y=${overlap.outputBottom}. ` +
              `Gap: ${overlap.gapBelowOutput}px`);
          }

          // Test keyboard interaction
          await page.locator('#inputField').tap();
          await page.waitForTimeout(500);
          await screenshot(page, `${deviceName}-07-keyboard-open`);

          // Check viewport after keyboard opens
          const viewportAfterKb = await page.evaluate(() => ({
            visualViewportHeight: window.visualViewport?.height,
            innerHeight: window.innerHeight,
            bodyScroll: window.scrollY,
          }));
          console.log(`  Viewport after keyboard:`, JSON.stringify(viewportAfterKb, null, 2));
        }

        // ===== 6. Test quick action buttons =====
        console.log('\n8. Testing quick action buttons...');
        const quickBtnCount = await page.locator('.quick-btn').count();
        console.log(`  Quick buttons: ${quickBtnCount}`);

        // Check if all quick buttons are visible and touchable
        for (let i = 0; i < quickBtnCount; i++) {
          const btn = page.locator('.quick-btn').nth(i);
          const box = await btn.boundingBox();
          if (box) {
            if (box.height < 44) {
              logIssue('minor', 'Quick button too small for touch',
                `Button ${i} height is ${box.height}px (Apple recommends 44px min)`);
            }
            if (box.width < 44) {
              logIssue('minor', 'Quick button too narrow for touch',
                `Button ${i} width is ${box.width}px`);
            }
          }
        }

        // Go back to dashboard
        await page.locator('#navBack').click();
        await page.waitForTimeout(500);
      }

      // ===== 7. Test various viewport issues =====
      console.log('\n9. Testing viewport and layout issues...');

      // Check for horizontal overflow
      const hOverflow = await page.evaluate(() => {
        const body = document.body;
        const docEl = document.documentElement;
        return {
          bodyScrollWidth: body.scrollWidth,
          bodyClientWidth: body.clientWidth,
          docScrollWidth: docEl.scrollWidth,
          docClientWidth: docEl.clientWidth,
          hasHorizontalScroll: body.scrollWidth > body.clientWidth || docEl.scrollWidth > docEl.clientWidth,
        };
      });
      console.log(`  Horizontal overflow:`, JSON.stringify(hOverflow, null, 2));
      if (hOverflow.hasHorizontalScroll) {
        logIssue('major', 'Horizontal scroll present on mobile',
          `Body scrollWidth (${hOverflow.bodyScrollWidth}) > clientWidth (${hOverflow.bodyClientWidth})`);
      }

      // Check viewport meta
      const viewportMeta = await page.evaluate(() => {
        const meta = document.querySelector('meta[name="viewport"]');
        return meta ? meta.content : 'NOT FOUND';
      });
      console.log(`  Viewport meta: ${viewportMeta}`);

      // Check if 100dvh works correctly
      const dvhCheck = await page.evaluate(() => ({
        innerHeight: window.innerHeight,
        dvh100: CSS.supports('height', '100dvh'),
      }));
      console.log(`  dvh support:`, JSON.stringify(dvhCheck, null, 2));
    }

    // ===== 8. Check for console errors =====
    if (consoleErrors.length > 0) {
      console.log('\n10. Console errors detected:');
      consoleErrors.forEach(e => console.log(`  - ${e}`));
      logIssue('major', 'JavaScript console errors', consoleErrors.join('\n'));
    }

    await screenshot(page, `${deviceName}-final`);
    await context.close();
  }

  await browser.close();

  // ===== Print summary =====
  console.log('\n\n========================================');
  console.log('  QA SUMMARY');
  console.log('========================================\n');

  if (issues.length === 0) {
    console.log('No issues found!\n');
  } else {
    const critical = issues.filter(i => i.severity === 'critical');
    const major = issues.filter(i => i.severity === 'major');
    const minor = issues.filter(i => i.severity === 'minor');
    const info = issues.filter(i => i.severity === 'info');

    if (critical.length) {
      console.log(`CRITICAL (${critical.length}):`);
      critical.forEach(i => console.log(`  - ${i.title}: ${i.detail}`));
    }
    if (major.length) {
      console.log(`MAJOR (${major.length}):`);
      major.forEach(i => console.log(`  - ${i.title}: ${i.detail}`));
    }
    if (minor.length) {
      console.log(`MINOR (${minor.length}):`);
      minor.forEach(i => console.log(`  - ${i.title}: ${i.detail}`));
    }
    if (info.length) {
      console.log(`INFO (${info.length}):`);
      info.forEach(i => console.log(`  - ${i.title}: ${i.detail}`));
    }
  }

  console.log(`\nScreenshots saved to: ${SCREENSHOT_DIR}`);
  console.log(`Total issues: ${issues.length}`);

  // Write issues to JSON for programmatic use
  writeFileSync(join(SCREENSHOT_DIR, 'qa-results.json'), JSON.stringify({ issues, timestamp: new Date().toISOString() }, null, 2));
}

main().catch(e => {
  console.error('QA failed:', e);
  process.exit(1);
});
