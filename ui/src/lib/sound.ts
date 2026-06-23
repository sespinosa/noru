// Runtime-synthesized UI cues via the Web Audio API — no bundled audio assets.
// Plays even while the window is minimized to the tray (the webview stays
// alive). Deliberately short, soft, and synth-flavored (R2-D2-ish bleeps).

let ctx: AudioContext | null = null;

function audioContext(): AudioContext | null {
  try {
    if (!ctx) {
      const Ctor =
        window.AudioContext ||
        (window as unknown as { webkitAudioContext?: typeof AudioContext })
          .webkitAudioContext;
      if (!Ctor) return null;
      ctx = new Ctor();
    }
    // Autoplay policy can leave the context suspended until a gesture; resume
    // is a no-op once running.
    if (ctx.state === "suspended") void ctx.resume();
    return ctx;
  } catch {
    return null;
  }
}

/** One short tone. `delay` lets callers sequence a small arpeggio. */
function tone(freq: number, startAt: number, durSec: number, gain = 0.06): void {
  const ac = audioContext();
  if (!ac) return;
  const osc = ac.createOscillator();
  const env = ac.createGain();
  osc.type = "sine";
  osc.frequency.value = freq;
  const t0 = ac.currentTime + startAt;
  env.gain.setValueAtTime(0, t0);
  env.gain.linearRampToValueAtTime(gain, t0 + 0.01);
  env.gain.exponentialRampToValueAtTime(0.0001, t0 + durSec);
  osc.connect(env).connect(ac.destination);
  osc.start(t0);
  osc.stop(t0 + durSec + 0.02);
}

/** Rising two-note "transcript ready" chirp. */
export function playDone(): void {
  tone(660, 0, 0.12);
  tone(990, 0.1, 0.16);
}

/** Low descending "something went wrong" blip. */
export function playError(): void {
  tone(440, 0, 0.14);
  tone(300, 0.12, 0.2);
}
