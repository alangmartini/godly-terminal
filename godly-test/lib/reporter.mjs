// lib/reporter.mjs
// Maestro-style real-time terminal output with checkmarks, colors, and timing.

// ANSI escape codes
const ESC = '\x1b[';
const RESET = `${ESC}0m`;
const BOLD = `${ESC}1m`;
const DIM = `${ESC}2m`;
const RED = `${ESC}31m`;
const GREEN = `${ESC}32m`;
const YELLOW = `${ESC}33m`;
const BLUE = `${ESC}34m`;
const CYAN = `${ESC}36m`;
const WHITE = `${ESC}37m`;
const BG_RED = `${ESC}41m`;
const BG_GREEN = `${ESC}42m`;

export class Reporter {
  constructor({ verbose = false, noColor = false } = {}) {
    this.verbose = verbose;
    this.noColor = noColor;
    this.totalPassed = 0;
    this.totalFailed = 0;
    this.totalSkipped = 0;
    this.totalSteps = 0;
    this.fileResults = [];
  }

  _c(code, text) {
    if (this.noColor) return text;
    return `${code}${text}${RESET}`;
  }

  header() {
    console.log('');
    console.log(this._c(BOLD + CYAN, ' godly-test v0.1.0'));
    console.log('');
  }

  fileStart(testName) {
    console.log(this._c(BOLD + WHITE, ` ${testName}`));
  }

  stepStart(stepIndex, totalSteps, label) {
    const counter = `[${stepIndex + 1}/${totalSteps}]`;
    const line = `   ${this._c(DIM, counter)} ${label}`;
    if (!this.verbose) {
      process.stdout.write(line);
    }
  }

  stepPass(stepIndex, totalSteps, label, durationMs) {
    const counter = `[${stepIndex + 1}/${totalSteps}]`;
    const time = formatDuration(durationMs);
    if (!this.verbose) {
      // Clear line and rewrite
      process.stdout.write('\r\x1b[K');
    }
    console.log(
      `   ${this._c(DIM, counter)} ${label}` +
      `${' '.repeat(Math.max(1, 40 - label.length))}` +
      `${this._c(GREEN, '\u2713')}  ${this._c(DIM, time)}`
    );
    this.totalPassed++;
    this.totalSteps++;
  }

  stepFail(stepIndex, totalSteps, label, durationMs, error) {
    const counter = `[${stepIndex + 1}/${totalSteps}]`;
    const time = formatDuration(durationMs);
    if (!this.verbose) {
      process.stdout.write('\r\x1b[K');
    }
    console.log(
      `   ${this._c(DIM, counter)} ${label}` +
      `${' '.repeat(Math.max(1, 40 - label.length))}` +
      `${this._c(RED, '\u2717')}  ${this._c(DIM, time)}`
    );
    // Error details indented
    const msg = error?.message || String(error);
    for (const line of msg.split('\n')) {
      console.log(`      ${this._c(RED, line)}`);
    }
    this.totalFailed++;
    this.totalSteps++;
  }

  stepSkip(stepIndex, totalSteps, label) {
    const counter = `[${stepIndex + 1}/${totalSteps}]`;
    if (!this.verbose) {
      process.stdout.write('\r\x1b[K');
    }
    console.log(
      `   ${this._c(DIM, counter)} ${label}` +
      `${' '.repeat(Math.max(1, 40 - label.length))}` +
      `${this._c(YELLOW, '-  skipped')}`
    );
    this.totalSkipped++;
    this.totalSteps++;
  }

  fileEnd(testName, passed, failed, durationMs) {
    this.fileResults.push({ testName, passed, failed, durationMs });
    console.log('');
  }

  screenshotCaptured(path) {
    console.log(`      ${this._c(DIM, `screenshot: ${path}`)}`);
  }

  summary(totalDurationMs) {
    const line = '\u2500'.repeat(40);
    console.log(` ${this._c(DIM, line)}`);

    const passedFiles = this.fileResults.filter(f => f.failed === 0).length;
    const failedFiles = this.fileResults.filter(f => f.failed > 0).length;

    const parts = [];
    if (passedFiles > 0) parts.push(this._c(GREEN, `${passedFiles} passed`));
    if (failedFiles > 0) parts.push(this._c(RED, `${failedFiles} failed`));

    const time = formatDuration(totalDurationMs);
    console.log(` Results: ${parts.join(', ')} (${this.totalSteps} steps)`);
    console.log(` Duration: ${time}`);
    console.log('');

    return failedFiles === 0;
  }
}

function formatDuration(ms) {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}
