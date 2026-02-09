let audioContext: AudioContext | null = null;

function getAudioContext(): AudioContext {
  if (!audioContext) {
    audioContext = new AudioContext();
  }
  if (audioContext.state === 'suspended') {
    audioContext.resume();
  }
  return audioContext;
}

export type SoundPreset =
  | 'chime' | 'bell' | 'ping'
  | 'soft-rise' | 'crystal' | 'bubble'
  | 'harp' | 'marimba' | 'cosmic' | 'droplet';

export function playNotificationSound(
  preset: SoundPreset = 'chime',
  volume: number = 0.5,
): void {
  const ctx = getAudioContext();
  const gainNode = ctx.createGain();
  gainNode.gain.value = Math.max(0, Math.min(1, volume));
  gainNode.connect(ctx.destination);

  switch (preset) {
    case 'chime':
      playChime(ctx, gainNode);
      break;
    case 'bell':
      playBell(ctx, gainNode);
      break;
    case 'ping':
      playPing(ctx, gainNode);
      break;
    case 'soft-rise':
      playSoftRise(ctx, gainNode);
      break;
    case 'crystal':
      playCrystal(ctx, gainNode);
      break;
    case 'bubble':
      playBubble(ctx, gainNode);
      break;
    case 'harp':
      playHarp(ctx, gainNode);
      break;
    case 'marimba':
      playMarimba(ctx, gainNode);
      break;
    case 'cosmic':
      playCosmic(ctx, gainNode);
      break;
    case 'droplet':
      playDroplet(ctx, gainNode);
      break;
  }
}

/** Two-tone chime: C5 then E5 */
function playChime(ctx: AudioContext, gain: GainNode): void {
  const now = ctx.currentTime;

  // First tone: C5 (523 Hz)
  const osc1 = ctx.createOscillator();
  osc1.type = 'sine';
  osc1.frequency.value = 523;
  const env1 = ctx.createGain();
  env1.gain.setValueAtTime(1, now);
  env1.gain.exponentialRampToValueAtTime(0.01, now + 0.15);
  osc1.connect(env1);
  env1.connect(gain);
  osc1.start(now);
  osc1.stop(now + 0.15);

  // Second tone: E5 (659 Hz)
  const osc2 = ctx.createOscillator();
  osc2.type = 'sine';
  osc2.frequency.value = 659;
  const env2 = ctx.createGain();
  env2.gain.setValueAtTime(1, now + 0.12);
  env2.gain.exponentialRampToValueAtTime(0.01, now + 0.35);
  osc2.connect(env2);
  env2.connect(gain);
  osc2.start(now + 0.12);
  osc2.stop(now + 0.35);
}

/** Damped sine bell */
function playBell(ctx: AudioContext, gain: GainNode): void {
  const now = ctx.currentTime;
  const osc = ctx.createOscillator();
  osc.type = 'sine';
  osc.frequency.value = 800;
  const env = ctx.createGain();
  env.gain.setValueAtTime(1, now);
  env.gain.exponentialRampToValueAtTime(0.01, now + 0.6);
  osc.connect(env);
  env.connect(gain);
  osc.start(now);
  osc.stop(now + 0.6);
}

/** Quick blip */
function playPing(ctx: AudioContext, gain: GainNode): void {
  const now = ctx.currentTime;
  const osc = ctx.createOscillator();
  osc.type = 'triangle';
  osc.frequency.value = 1200;
  osc.frequency.exponentialRampToValueAtTime(600, now + 0.08);
  const env = ctx.createGain();
  env.gain.setValueAtTime(1, now);
  env.gain.exponentialRampToValueAtTime(0.01, now + 0.1);
  osc.connect(env);
  env.connect(gain);
  osc.start(now);
  osc.stop(now + 0.1);
}

/** Gentle 3-note arpeggio: C5→E5→G5 */
function playSoftRise(ctx: AudioContext, gain: GainNode): void {
  const now = ctx.currentTime;
  const notes = [523, 659, 784]; // C5, E5, G5
  notes.forEach((freq, i) => {
    const osc = ctx.createOscillator();
    osc.type = 'sine';
    osc.frequency.value = freq;
    const env = ctx.createGain();
    const start = now + i * 0.1;
    env.gain.setValueAtTime(0.01, start);
    env.gain.linearRampToValueAtTime(0.8, start + 0.03);
    env.gain.exponentialRampToValueAtTime(0.01, start + 0.25);
    osc.connect(env);
    env.connect(gain);
    osc.start(start);
    osc.stop(start + 0.25);
  });
}

/** High sparkle with harmonics */
function playCrystal(ctx: AudioContext, gain: GainNode): void {
  const now = ctx.currentTime;
  // Fundamental
  const osc1 = ctx.createOscillator();
  osc1.type = 'sine';
  osc1.frequency.value = 1400;
  const env1 = ctx.createGain();
  env1.gain.setValueAtTime(0.8, now);
  env1.gain.exponentialRampToValueAtTime(0.01, now + 0.3);
  osc1.connect(env1);
  env1.connect(gain);
  osc1.start(now);
  osc1.stop(now + 0.3);
  // Overtone at 2x
  const osc2 = ctx.createOscillator();
  osc2.type = 'sine';
  osc2.frequency.value = 2800;
  const env2 = ctx.createGain();
  env2.gain.setValueAtTime(0.4, now);
  env2.gain.exponentialRampToValueAtTime(0.01, now + 0.15);
  osc2.connect(env2);
  env2.connect(gain);
  osc2.start(now);
  osc2.stop(now + 0.15);
}

/** Bubbly pop with upward frequency sweep */
function playBubble(ctx: AudioContext, gain: GainNode): void {
  const now = ctx.currentTime;
  const osc = ctx.createOscillator();
  osc.type = 'sine';
  osc.frequency.setValueAtTime(300, now);
  osc.frequency.exponentialRampToValueAtTime(1200, now + 0.08);
  const env = ctx.createGain();
  env.gain.setValueAtTime(0.9, now);
  env.gain.exponentialRampToValueAtTime(0.01, now + 0.15);
  osc.connect(env);
  env.connect(gain);
  osc.start(now);
  osc.stop(now + 0.15);
}

/** Plucked string: sine + triangle layered */
function playHarp(ctx: AudioContext, gain: GainNode): void {
  const now = ctx.currentTime;
  const osc1 = ctx.createOscillator();
  osc1.type = 'sine';
  osc1.frequency.value = 660;
  const osc2 = ctx.createOscillator();
  osc2.type = 'triangle';
  osc2.frequency.value = 660;
  const env = ctx.createGain();
  env.gain.setValueAtTime(1, now);
  env.gain.exponentialRampToValueAtTime(0.3, now + 0.05);
  env.gain.exponentialRampToValueAtTime(0.01, now + 0.5);
  osc1.connect(env);
  osc2.connect(env);
  env.connect(gain);
  osc1.start(now);
  osc2.start(now);
  osc1.stop(now + 0.5);
  osc2.stop(now + 0.5);
}

/** Warm wooden tone with quick exponential decay */
function playMarimba(ctx: AudioContext, gain: GainNode): void {
  const now = ctx.currentTime;
  const osc = ctx.createOscillator();
  osc.type = 'sine';
  osc.frequency.value = 520;
  const env = ctx.createGain();
  env.gain.setValueAtTime(1, now);
  env.gain.exponentialRampToValueAtTime(0.01, now + 0.25);
  osc.connect(env);
  env.connect(gain);
  osc.start(now);
  osc.stop(now + 0.25);
}

/** Ethereal sci-fi sweep with two detuned oscillators */
function playCosmic(ctx: AudioContext, gain: GainNode): void {
  const now = ctx.currentTime;
  const osc1 = ctx.createOscillator();
  osc1.type = 'sine';
  osc1.frequency.setValueAtTime(400, now);
  osc1.frequency.linearRampToValueAtTime(600, now + 0.6);
  const osc2 = ctx.createOscillator();
  osc2.type = 'sine';
  osc2.frequency.setValueAtTime(404, now);
  osc2.frequency.linearRampToValueAtTime(596, now + 0.6);
  const env = ctx.createGain();
  env.gain.setValueAtTime(0.6, now);
  env.gain.exponentialRampToValueAtTime(0.01, now + 0.7);
  osc1.connect(env);
  osc2.connect(env);
  env.connect(gain);
  osc1.start(now);
  osc2.start(now);
  osc1.stop(now + 0.7);
  osc2.stop(now + 0.7);
}

/** Water drop with rapid downward pitch bend */
function playDroplet(ctx: AudioContext, gain: GainNode): void {
  const now = ctx.currentTime;
  const osc = ctx.createOscillator();
  osc.type = 'sine';
  osc.frequency.setValueAtTime(1800, now);
  osc.frequency.exponentialRampToValueAtTime(400, now + 0.1);
  const env = ctx.createGain();
  env.gain.setValueAtTime(0.9, now);
  env.gain.exponentialRampToValueAtTime(0.01, now + 0.2);
  osc.connect(env);
  env.connect(gain);
  osc.start(now);
  osc.stop(now + 0.2);
}
