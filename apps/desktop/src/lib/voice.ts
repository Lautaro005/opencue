// Voice conversations: record the mic, transcribe with ElevenLabs (through the core), send the
// text, and read the agent's reply aloud when it finishes.

import { api } from "./api";
import type { Message } from "./types";

let recorder: MediaRecorder | null = null;
let chunks: Blob[] = [];
let player: HTMLAudioElement | null = null;

/** Threads where the last user message was spoken, so the reply should be spoken too. */
const voiceThreads = new Set<string>();

export function isRecording() {
  return recorder?.state === "recording";
}

export async function startRecording(): Promise<void> {
  stopSpeaking();
  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  const mime = ["audio/webm;codecs=opus", "audio/webm", "audio/ogg", "audio/mp4"].find((m) => MediaRecorder.isTypeSupported(m)) ?? "";
  recorder = new MediaRecorder(stream, mime ? { mimeType: mime } : undefined);
  chunks = [];
  recorder.ondataavailable = (e) => e.data.size > 0 && chunks.push(e.data);
  recorder.start();
}

/** Stop and transcribe. Returns the recognized text (may be empty). */
export async function stopRecording(): Promise<string> {
  const rec = recorder;
  if (!rec) return "";
  const done = new Promise<void>((resolve) => (rec.onstop = () => resolve()));
  rec.stop();
  await done;
  rec.stream.getTracks().forEach((t) => t.stop());
  recorder = null;
  const blob = new Blob(chunks, { type: rec.mimeType || "audio/webm" });
  chunks = [];
  if (blob.size === 0) return "";
  const b64 = await blobToBase64(blob);
  return api.transcribe(b64, blob.type);
}

export function cancelRecording() {
  if (!recorder) return;
  recorder.onstop = null;
  recorder.stop();
  recorder.stream.getTracks().forEach((t) => t.stop());
  recorder = null;
  chunks = [];
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result).split(",")[1] ?? "");
    r.onerror = () => reject(r.error);
    r.readAsDataURL(blob);
  });
}

export function markVoiceThread(threadId: string) {
  voiceThreads.add(threadId);
}

export function stopSpeaking() {
  player?.pause();
  player = null;
}

export async function speak(text: string, agentId?: string) {
  const b64 = await api.speak(text, agentId);
  if (!b64) return;
  stopSpeaking();
  player = new Audio(`data:audio/mpeg;base64,${b64}`);
  await player.play();
}

/** Called for every message event: speaks finished replies in voice threads. */
export function onMessageForVoice(m: Message, autoSpeak: boolean) {
  if (!autoSpeak || !voiceThreads.has(m.thread_id)) return;
  if (m.role === "user") {
    voiceThreads.delete(m.thread_id);
    return;
  }
  if (m.role !== "assistant" || m.status !== "done" || !m.content.trim() || m.tool_calls?.length) return;
  voiceThreads.delete(m.thread_id);
  void speak(m.content, m.agent_id ?? undefined).catch(() => {});
}
