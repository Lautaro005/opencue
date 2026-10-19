// Voice calls: the user talks, the agent answers aloud, and it goes on like a phone call.
//
// Listening uses the microphone with a small voice-activity detector: speech starts a recording,
// about a second of silence ends it, and the audio is transcribed (ElevenLabs through the core, or
// the browser's own recognizer when there is no key). The text goes to the agent as a normal
// message in the chat; each finished reply is read aloud (ElevenLabs, or the system voice). Talking
// while the agent speaks interrupts it.

import { api } from "./api";
import { useStore } from "./store";
import type { Message } from "./types";

export type CallState = "connecting" | "listening" | "hearing" | "thinking" | "speaking" | "ended";

export interface CallEvents {
  state: (s: CallState) => void;
  /** What the user said, once transcribed. */
  heard: (text: string) => void;
  /** What the agent is saying. */
  saying: (text: string) => void;
  /** Mic level 0..1, for the animation. */
  level: (v: number) => void;
  error: (message: string) => void;
}

type Recognizer = {
  start(): void;
  stop(): void;
  abort(): void;
  lang: string;
  interimResults: boolean;
  continuous: boolean;
  onresult: ((e: { results: ArrayLike<ArrayLike<{ transcript: string }>> }) => void) | null;
  onend: (() => void) | null;
  onerror: ((e: { error: string }) => void) | null;
};

function browserRecognizer(): (new () => Recognizer) | null {
  const w = window as unknown as { SpeechRecognition?: new () => Recognizer; webkitSpeechRecognition?: new () => Recognizer };
  return w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null;
}

/** What is worth reading aloud: no Markdown marks, links, code blocks or tables. */
export function spokenText(md: string): string {
  return md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/https?:\/\/\S+/g, " ")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*\d+\.\s+/gm, "")
    .replace(/^#+\s*/gm, "")
    .replace(/[*_~>|]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

const SILENCE_MS = 1100;
const MIN_SPEECH_MS = 350;

export class Call {
  private stream: MediaStream | null = null;
  private ctx: AudioContext | null = null;
  private analyser: AnalyserNode | null = null;
  private recorder: MediaRecorder | null = null;
  private chunks: Blob[] = [];
  private timer: ReturnType<typeof setInterval> | null = null;
  private audio: HTMLAudioElement | null = null;
  private recognizer: Recognizer | null = null;
  private unsubscribe: (() => void) | null = null;
  private state: CallState = "connecting";
  private ended = false;
  private muted = false;
  private speakerOn = true;
  private floor = 0.01;
  private speechSince = 0;
  private silenceSince = 0;
  private recStart = 0;
  private useElevenLabs = false;
  private spoken = new Set<string>();
  private queue: Message[] = [];
  private playing = false;
  private waitingReply = false;

  constructor(
    private threadId: string,
    private agentId: string,
    private on: CallEvents,
  ) {}

  private set(s: CallState) {
    if (this.state === s || this.ended) return;
    this.state = s;
    this.on.state(s);
  }

  async start() {
    this.useElevenLabs = (await api.voiceSettings().catch(() => null))?.configured ?? false;
    const Rec = browserRecognizer();
    if (!this.useElevenLabs && !Rec) {
      throw new Error("noVoice");
    }
    await api.setCallMode(this.threadId, true).catch(() => {});
    // Whatever the agent had already said is not read out loud.
    for (const m of useStore.getState().messages[this.threadId] ?? []) this.spoken.add(m.id);
    this.unsubscribe = useStore.subscribe((s) => this.onStore(s.messages[this.threadId] ?? [], (s.working[this.threadId] ?? []).length > 0));
    if (this.useElevenLabs) {
      this.stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true } });
      this.ctx = new AudioContext();
      const src = this.ctx.createMediaStreamSource(this.stream);
      this.analyser = this.ctx.createAnalyser();
      this.analyser.fftSize = 1024;
      src.connect(this.analyser);
      await this.calibrate();
      this.timer = setInterval(() => this.tick(), 50);
    } else {
      this.listenWithBrowser();
    }
    this.set("listening");
  }

  setMuted(v: boolean) {
    this.muted = v;
    if (v) {
      this.abortRecording();
      this.recognizer?.abort();
      this.set("listening");
    } else if (!this.useElevenLabs && this.state !== "speaking" && this.state !== "thinking") {
      this.listenWithBrowser();
    }
  }

  setSpeaker(v: boolean) {
    this.speakerOn = v;
    if (!v) this.stopAudio();
  }

  async end() {
    if (this.ended) return;
    this.ended = true;
    this.on.state("ended");
    if (this.timer) clearInterval(this.timer);
    this.unsubscribe?.();
    this.abortRecording();
    this.recognizer?.abort();
    this.stopAudio();
    this.stream?.getTracks().forEach((t) => t.stop());
    void this.ctx?.close().catch(() => {});
    await api.setCallMode(this.threadId, false).catch(() => {});
  }

  // ----- listening -----

  private level(): number {
    if (!this.analyser) return 0;
    const buf = new Uint8Array(this.analyser.fftSize);
    this.analyser.getByteTimeDomainData(buf);
    let sum = 0;
    for (const v of buf) {
      const x = (v - 128) / 128;
      sum += x * x;
    }
    return Math.sqrt(sum / buf.length);
  }

  private async calibrate() {
    const samples: number[] = [];
    for (let i = 0; i < 10; i++) {
      await new Promise((r) => setTimeout(r, 50));
      samples.push(this.level());
    }
    this.floor = Math.max(0.008, samples.reduce((a, b) => a + b, 0) / samples.length);
  }

  private tick() {
    if (this.ended) return;
    const lvl = this.level();
    this.on.level(Math.min(1, lvl * 8));
    if (this.muted) return;
    const speaking = this.state === "speaking";
    // While the agent talks, only clearly louder sounds count (its own voice leaks into the mic).
    const threshold = Math.max(this.floor * (speaking ? 6 : 3), speaking ? 0.06 : 0.02);
    const now = performance.now();
    const loud = lvl > threshold;
    if (this.recorder) {
      if (loud) this.silenceSince = 0;
      else if (!this.silenceSince) this.silenceSince = now;
      else if (now - this.silenceSince > SILENCE_MS) void this.finishRecording();
      return;
    }
    if (this.state === "thinking") return;
    if (loud) {
      if (!this.speechSince) this.speechSince = now;
      else if (now - this.speechSince > (speaking ? 300 : 150)) {
        this.speechSince = 0;
        if (speaking) this.interrupt();
        this.beginRecording();
      }
    } else {
      this.speechSince = 0;
    }
  }

  private beginRecording() {
    if (!this.stream) return;
    const mime = ["audio/webm;codecs=opus", "audio/webm", "audio/ogg", "audio/mp4"].find((m) => MediaRecorder.isTypeSupported(m)) ?? "";
    this.recorder = new MediaRecorder(this.stream, mime ? { mimeType: mime } : undefined);
    this.chunks = [];
    this.recorder.ondataavailable = (e) => e.data.size > 0 && this.chunks.push(e.data);
    this.recorder.start();
    this.recStart = performance.now();
    this.silenceSince = 0;
    this.set("hearing");
  }

  private abortRecording() {
    const rec = this.recorder;
    if (!rec) return;
    rec.onstop = null;
    try {
      rec.stop();
    } catch {
      /* already stopped */
    }
    this.recorder = null;
    this.chunks = [];
  }

  private async finishRecording() {
    const rec = this.recorder;
    if (!rec) return;
    this.recorder = null;
    const long = performance.now() - this.recStart - SILENCE_MS > MIN_SPEECH_MS;
    const done = new Promise<void>((resolve) => (rec.onstop = () => resolve()));
    rec.stop();
    await done;
    const blob = new Blob(this.chunks, { type: rec.mimeType || "audio/webm" });
    this.chunks = [];
    if (!long || blob.size === 0) {
      this.set("listening");
      return;
    }
    this.set("thinking");
    try {
      const b64 = await new Promise<string>((resolve, reject) => {
        const r = new FileReader();
        r.onload = () => resolve(String(r.result).split(",")[1] ?? "");
        r.onerror = () => reject(r.error);
        r.readAsDataURL(blob);
      });
      const text = (await api.transcribe(b64, blob.type)).trim();
      if (!text) {
        this.set("listening");
        return;
      }
      this.say(text);
    } catch (e) {
      this.on.error(String(e));
      this.set("listening");
    }
  }

  private listenWithBrowser() {
    const Rec = browserRecognizer();
    if (!Rec || this.ended || this.muted) return;
    const r = new Rec();
    r.lang = navigator.language || "en-US";
    r.interimResults = false;
    r.continuous = false;
    let got = false;
    r.onresult = (e) => {
      const text = Array.from(e.results)
        .map((x) => x[0]?.transcript ?? "")
        .join(" ")
        .trim();
      if (text) {
        got = true;
        this.say(text);
      }
    };
    r.onerror = (e) => {
      if (e.error === "not-allowed" || e.error === "service-not-allowed") this.on.error("The microphone is blocked.");
    };
    r.onend = () => {
      if (!got && !this.ended && !this.muted && this.state !== "thinking" && this.state !== "speaking") setTimeout(() => this.listenWithBrowser(), 250);
    };
    this.recognizer = r;
    this.set("listening");
    try {
      r.start();
    } catch {
      /* already started */
    }
  }

  // ----- talking with the agent -----

  private say(text: string) {
    this.on.heard(text);
    this.waitingReply = true;
    this.set("thinking");
    void useStore.getState().send(this.threadId, text);
  }

  private onStore(messages: Message[], working: boolean) {
    if (this.ended) return;
    for (const m of messages) {
      if (this.spoken.has(m.id) || m.role !== "assistant" || m.status !== "done" || m.tool_calls?.length) continue;
      this.spoken.add(m.id);
      const text = spokenText(m.content);
      if (text) this.queue.push({ ...m, content: text });
    }
    if (this.queue.length && !this.playing) void this.playNext();
    if (!working && !this.queue.length && !this.playing && this.waitingReply && this.state === "thinking") {
      this.waitingReply = false;
      this.set("listening");
      if (!this.useElevenLabs) this.listenWithBrowser();
    }
  }

  private async playNext() {
    const m = this.queue.shift();
    if (!m || this.ended) return;
    this.playing = true;
    this.recognizer?.abort();
    this.set("speaking");
    this.on.saying(m.content);
    try {
      if (this.speakerOn) await this.speak(m.content);
    } catch (e) {
      this.on.error(String(e));
    }
    this.playing = false;
    if (this.ended) return;
    if (this.queue.length) {
      void this.playNext();
      return;
    }
    const busy = (useStore.getState().working[this.threadId] ?? []).length > 0;
    if (busy) {
      this.set("thinking");
    } else {
      this.waitingReply = false;
      this.set("listening");
      if (!this.useElevenLabs) this.listenWithBrowser();
    }
  }

  private async speak(text: string) {
    if (this.useElevenLabs) {
      const b64 = await api.speak(text, this.agentId);
      if (!b64 || this.ended) return;
      const audio = new Audio(`data:audio/mpeg;base64,${b64}`);
      this.audio = audio;
      await new Promise<void>((resolve) => {
        audio.onended = () => resolve();
        audio.onpause = () => resolve();
        audio.onerror = () => resolve();
        audio.play().catch(() => resolve());
      });
      this.audio = null;
      return;
    }
    await new Promise<void>((resolve) => {
      const u = new SpeechSynthesisUtterance(text);
      u.lang = navigator.language || "en-US";
      u.onend = () => resolve();
      u.onerror = () => resolve();
      window.speechSynthesis.speak(u);
    });
  }

  private stopAudio() {
    this.audio?.pause();
    this.audio = null;
    window.speechSynthesis?.cancel();
  }

  /** The user talked over the agent. */
  private interrupt() {
    this.queue = [];
    this.stopAudio();
  }
}
