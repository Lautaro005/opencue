// Audio helpers shared by the mic button and voice calls.

import { api } from "./api";

/** Any recording (webm/opus, mp4/aac…) as 16 kHz mono 16-bit WAV, which every speech engine reads. */
export async function toWav16k(blob: Blob): Promise<Blob> {
  const ctx = new AudioContext();
  try {
    const decoded = await ctx.decodeAudioData(await blob.arrayBuffer());
    const rate = 16000;
    const frames = Math.max(1, Math.ceil(decoded.duration * rate));
    const off = new OfflineAudioContext(1, frames, rate);
    const src = off.createBufferSource();
    src.buffer = decoded;
    src.connect(off.destination);
    src.start();
    const pcm = (await off.startRendering()).getChannelData(0);
    const out = new DataView(new ArrayBuffer(44 + pcm.length * 2));
    const str = (o: number, s: string) => [...s].forEach((c, i) => out.setUint8(o + i, c.charCodeAt(0)));
    str(0, "RIFF");
    out.setUint32(4, 36 + pcm.length * 2, true);
    str(8, "WAVEfmt ");
    out.setUint32(16, 16, true);
    out.setUint16(20, 1, true);
    out.setUint16(22, 1, true);
    out.setUint32(24, rate, true);
    out.setUint32(28, rate * 2, true);
    out.setUint16(32, 2, true);
    out.setUint16(34, 16, true);
    str(36, "data");
    out.setUint32(40, pcm.length * 2, true);
    pcm.forEach((v, i) => out.setInt16(44 + i * 2, Math.max(-1, Math.min(1, v)) * 0x7fff, true));
    return new Blob([out.buffer], { type: "audio/wav" });
  } finally {
    void ctx.close().catch(() => {});
  }
}

export function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result).split(",")[1] ?? "");
    r.onerror = () => reject(r.error);
    r.readAsDataURL(blob);
  });
}

/** Text from a recording, with whichever engine Settings → Voice picked (ElevenLabs or local models). */
export async function transcribeBlob(blob: Blob): Promise<string> {
  let send = blob;
  try {
    send = await toWav16k(blob);
  } catch {
    /* send the original recording */
  }
  return api.transcribe(await blobToBase64(send), send.type);
}

/** Play audio the core spoke (an `{ mime, data }` pair); resolves when it ends or is stopped. */
export function playSpeech(speech: { mime: string; data: string }, hold: (a: HTMLAudioElement) => void): Promise<void> {
  const audio = new Audio(`data:${speech.mime};base64,${speech.data}`);
  hold(audio);
  return new Promise<void>((resolve) => {
    audio.onended = () => resolve();
    audio.onpause = () => resolve();
    audio.onerror = () => resolve();
    audio.play().catch(() => resolve());
  });
}
