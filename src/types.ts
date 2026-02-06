export interface WhisperConfig {
  modelPath: string;
  language: string;
  maxDuration: number;
  silenceDuration: number;
  silenceThreshold: string;
}

export interface RecordingOptions {
  maxDuration: number;
  silenceDuration: number;
  silenceThreshold: string;
}

export interface RecordingResult {
  filePath: string;
  durationMs: number;
}

export interface TranscriptionOptions {
  modelPath: string;
  language: string;
}

export interface TranscriptionResult {
  text: string;
}
