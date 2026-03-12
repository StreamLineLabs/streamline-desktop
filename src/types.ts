// Shared types used across components

import type React from "react";

export type Tab = "dashboard" | "topics" | "produce" | "consume" | "groups" | "schemas" | "settings";

export interface Toast {
  id: number;
  message: string;
  type: "error" | "success" | "info";
}

export interface ServerStatus {
  running: boolean;
  pid?: number;
  kafka_port: number;
  http_port: number;
}

export interface TopicInfo {
  name: string;
  partitions: number;
  messages: number;
}

export interface ServerInfo {
  version: string;
  uptime: number;
  topics: number;
  messages: number;
}

export interface Settings {
  kafkaPort: number;
  httpPort: number;
  dataDir: string;
  logLevel: string;
}

export interface ConsumerGroupInfo {
  group_id: string;
  state: string;
  members: number;
  topics: string[];
}

export interface ConsumerGroupDetail {
  group_id: string;
  state: string;
  protocol: string;
  members: GroupMember[];
  offsets: GroupOffset[];
}

export interface GroupMember {
  member_id: string;
  client_id: string;
  host: string;
  assignments: string[];
}

export interface GroupOffset {
  topic: string;
  partition: number;
  current_offset: number;
  log_end_offset: number;
  lag: number;
}

export interface SchemaSubject {
  subject: string;
  version: number;
  schema_type: string;
}

export interface SchemaDetail {
  subject: string;
  version: number;
  id: number;
  schema_type: string;
  schema: string;
  compatibility: string;
}

export const COLORS = {
  bg: "#0f0f23",
  sidebar: "#1a1a2e",
  active: "#16213e",
  card: "#1a1a2e",
  border: "#2a2a4a",
  text: "#eee",
  textDim: "#888",
  green: "#4caf50",
  red: "#f44336",
  blue: "#2196f3",
  yellow: "#ff9800",
  purple: "#9c27b0",
} as const;

export const IS_TAURI = !!(window as any).__TAURI__?.core?.invoke;

export const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  (window as any).__TAURI__?.core?.invoke ??
  (async (cmd: string) => {
    console.warn(`[Streamline Desktop] Tauri not available — "${cmd}" returns mock data`);
    if (cmd === "get_server_status") return { running: false, kafka_port: 9092, http_port: 9094 };
    if (cmd === "get_topics") return [];
    if (cmd === "get_server_info") return { version: "0.2.0", uptime: 0, topics: 0, messages: 0 };
    return {};
  });

// Shared styles
export const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "8px 12px",
  borderRadius: 6,
  border: `1px solid ${COLORS.border}`,
  background: COLORS.bg,
  color: COLORS.text,
  fontSize: 14,
  boxSizing: "border-box",
  outline: "none",
};

export const btnStyle: React.CSSProperties = {
  padding: "8px 20px",
  borderRadius: 6,
  background: COLORS.blue,
  color: "#fff",
  border: "none",
  cursor: "pointer",
  fontSize: 14,
  fontWeight: 600,
  whiteSpace: "nowrap",
};

export const thStyle: React.CSSProperties = {
  padding: "12px 16px",
  fontSize: 12,
  color: COLORS.textDim,
  fontWeight: 600,
  textTransform: "uppercase",
};

export const tdStyle: React.CSSProperties = { padding: "10px 16px" };
