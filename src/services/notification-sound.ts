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

export type SoundPreset = 'chime' | 'bell' | 'ping';

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
