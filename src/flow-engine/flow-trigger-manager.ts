import type { Flow, FlowNode } from './types';
import { chordToString, eventToChord, type KeyChord } from '../state/keybinding-store';

// ── Flow Trigger Manager ────────────────────────────────────────────

/**
 * Manages hotkey triggers for flows. When a flow has a `trigger.hotkey`
 * node, its chord is registered with a global keydown listener. When a
 * matching key combo is pressed, the provided `onTrigger` callback is
 * invoked with the flow's ID.
 */
export class FlowTriggerManager {
  /** chordString -> { flowId, chord } */
  private hotkeys: Map<string, { flowId: string; chord: KeyChord }> = new Map();
  private onTrigger: ((flowId: string) => void) | null = null;
  private keydownHandler: ((e: KeyboardEvent) => void) | null = null;

  /**
   * Start listening for hotkey triggers. Must be called once during
   * initialization.
   */
  start(onTrigger: (flowId: string) => void): void {
    this.onTrigger = onTrigger;

    this.keydownHandler = (e: KeyboardEvent) => {
      // Ignore bare modifier-only key presses
      if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;

      const chord = eventToChord(e);
      const chordStr = chordToString(chord);
      const entry = this.hotkeys.get(chordStr);

      if (entry && this.onTrigger) {
        e.preventDefault();
        e.stopPropagation();
        this.onTrigger(entry.flowId);
      }
    };

    document.addEventListener('keydown', this.keydownHandler, { capture: true });
  }

  /** Stop listening and clear all registered hotkeys. */
  stop(): void {
    if (this.keydownHandler) {
      document.removeEventListener('keydown', this.keydownHandler, { capture: true });
      this.keydownHandler = null;
    }
    this.hotkeys.clear();
    this.onTrigger = null;
  }

  /**
   * Register all trigger.hotkey nodes from a flow. Extracts the chord
   * from the node's config and stores it in the hotkey map.
   */
  registerFlow(flow: Flow): void {
    if (!flow.enabled) return;

    for (const node of flow.nodes) {
      if (node.type === 'trigger.hotkey' && !node.disabled) {
        const chord = this.extractChord(node);
        if (chord) {
          const chordStr = chordToString(chord);
          this.hotkeys.set(chordStr, { flowId: flow.id, chord });
        }
      }
    }
  }

  /** Remove all hotkey registrations for a given flow. */
  unregisterFlow(flowId: string): void {
    const toRemove: string[] = [];
    for (const [chordStr, entry] of this.hotkeys) {
      if (entry.flowId === flowId) {
        toRemove.push(chordStr);
      }
    }
    for (const key of toRemove) {
      this.hotkeys.delete(key);
    }
  }

  /** Clear and re-register all enabled flows. */
  refreshAll(flows: Flow[]): void {
    this.hotkeys.clear();
    for (const flow of flows) {
      this.registerFlow(flow);
    }
  }

  // ── Internal ────────────────────────────────────────────────────

  /**
   * Extract a KeyChord from a trigger.hotkey node's config.
   * Expected config shape: { chord: { ctrlKey, shiftKey, altKey, key } }
   */
  private extractChord(node: FlowNode): KeyChord | null {
    const config = node.config;
    if (!config || typeof config !== 'object') return null;

    const chord = config.chord;
    if (!chord || typeof chord !== 'object') return null;

    const c = chord as Record<string, unknown>;
    if (typeof c.key !== 'string' || !c.key) return null;

    return {
      ctrlKey: Boolean(c.ctrlKey),
      shiftKey: Boolean(c.shiftKey),
      altKey: Boolean(c.altKey),
      key: (c.key as string).toLowerCase(),
    };
  }
}
