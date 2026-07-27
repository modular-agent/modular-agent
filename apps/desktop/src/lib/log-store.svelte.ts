import {
  attachLogger,
  debug as logDebug,
  error as logError,
  info as logInfo,
  warn as logWarn,
} from "@tauri-apps/plugin-log";

export const enum LogLevel {
  Trace = 1,
  Debug = 2,
  Info = 3,
  Warn = 4,
  Error = 5,
}

export interface LogEntry {
  id: number;
  level: LogLevel;
  message: string;
  timestamp: Date;
}

const MAX_LOG_ENTRIES = 1000;
const MAX_MESSAGE_LENGTH = 10_000;

function argsToString(args: any[]): string {
  return args
    .map((arg) => {
      if (typeof arg === "string") return arg;
      try {
        return JSON.stringify(arg);
      } catch {
        return String(arg);
      }
    })
    .join(" ");
}

function truncate(str: string): string {
  if (str.length <= MAX_MESSAGE_LENGTH) return str;
  return str.slice(0, MAX_MESSAGE_LENGTH) + "...(truncated)";
}

function pluginLevelToLogLevel(level: number): LogLevel {
  // tauri-plugin-log JS: Trace=1, Debug=2, Info=3, Warn=4, Error=5
  return level as LogLevel;
}

class LogStore {
  entries: LogEntry[] = $state([]);
  private nextId = 0;

  addEntry(level: LogLevel, message: string) {
    this.entries.push({
      id: this.nextId++,
      level,
      message: truncate(message),
      timestamp: new Date(),
    });
    if (this.entries.length > MAX_LOG_ENTRIES) {
      this.entries.splice(0, this.entries.length - MAX_LOG_ENTRIES);
    }
  }

  clear() {
    this.entries = [];
  }
}

export const logStore = new LogStore();

let initialized = false;

export async function initLogging(): Promise<() => void> {
  if (initialized) return () => {};
  if (typeof window === "undefined" || !(window as any).__TAURI_INTERNALS__) {
    return () => {};
  }
  initialized = true;

  // Save original console methods before patching
  const originalLog = console.log;
  const originalInfo = console.info;
  const originalWarn = console.warn;
  const originalError = console.error;
  const originalDebug = console.debug;

  // 1. Attach logger to capture all log events (Rust + forwarded JS) into the store
  const unlisten = await attachLogger(({ level, message }) => {
    try {
      logStore.addEntry(pluginLevelToLogLevel(level), message);
      // Also print to browser DevTools using ORIGINAL console (avoids infinite loop)
      switch (level) {
        case 1:
          originalLog("[TRACE]", message);
          break;
        case 2:
          originalDebug(message);
          break;
        case 3:
          originalInfo(message);
          break;
        case 4:
          originalWarn(message);
          break;
        case 5:
          originalError(message);
          break;
      }
    } catch {
      // Prevent errors in logger from breaking the event listener
    }
  });

  // 2. Monkey-patch browser console to forward to Rust log system
  let forwarding = false;

  console.log = (...args: any[]) => {
    originalLog.apply(console, args);
    if (!forwarding) {
      forwarding = true;
      logInfo(argsToString(args))
        .catch(() => {})
        .finally(() => {
          forwarding = false;
        });
    }
  };

  console.info = (...args: any[]) => {
    originalInfo.apply(console, args);
    if (!forwarding) {
      forwarding = true;
      logInfo(argsToString(args))
        .catch(() => {})
        .finally(() => {
          forwarding = false;
        });
    }
  };

  console.warn = (...args: any[]) => {
    originalWarn.apply(console, args);
    if (!forwarding) {
      forwarding = true;
      logWarn(argsToString(args))
        .catch(() => {})
        .finally(() => {
          forwarding = false;
        });
    }
  };

  console.error = (...args: any[]) => {
    originalError.apply(console, args);
    if (!forwarding) {
      forwarding = true;
      logError(argsToString(args))
        .catch(() => {})
        .finally(() => {
          forwarding = false;
        });
    }
  };

  console.debug = (...args: any[]) => {
    originalDebug.apply(console, args);
    if (!forwarding) {
      forwarding = true;
      logDebug(argsToString(args))
        .catch(() => {})
        .finally(() => {
          forwarding = false;
        });
    }
  };

  // Return cleanup function
  return () => {
    unlisten();
    console.log = originalLog;
    console.info = originalInfo;
    console.warn = originalWarn;
    console.error = originalError;
    console.debug = originalDebug;
    initialized = false;
  };
}
