// Shared types used across components

import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
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

export interface ConsumedMessage {
  key: string;
  value: string;
  partition: number;
  offset: number;
}

export interface ServerInfo {
  version: string;
  uptime_secs: number;
  kafka_port: number;
  http_port: number;
}

export interface Settings {
  host: string;
  kafkaPort: number;
  httpPort: number;
  dataDir: string;
  logLevel: string;
}

export interface ServerSettingsPayload {
  host: string;
  kafka_port: number;
  http_port: number;
  data_dir: string;
  log_level: string;
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

export const IS_TAURI = isTauri();

export const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  IS_TAURI
    ? tauriInvoke
    :
  (async (cmd: string, args?: Record<string, unknown>) => {
    console.warn(`[Streamline Desktop] Tauri not available — "${cmd}" returns preview data`);
    switch (cmd) {
      case "get_server_status":
        return { running: false, pid: null, kafka_port: 9092, http_port: 9094 };
      case "get_topics":
        return [
          { name: "demo-events", partitions: 3, messages: 142 },
          { name: "user-signups", partitions: 1, messages: 38 },
          { name: "orders", partitions: 6, messages: 1024 },
        ];
      case "get_server_info":
        return { version: "0.2.0 (preview)", uptime_secs: 0, kafka_port: 9092, http_port: 9094 };
      case "list_consumer_groups":
        return [
          { group_id: "analytics-pipeline", state: "Stable", members: 3, topics: ["demo-events"] },
          { group_id: "order-processor", state: "Stable", members: 1, topics: ["orders"] },
        ];
      case "list_schemas":
        return [
          { subject: "demo-events-value", version: 1, schema_type: "JSON" },
          { subject: "orders-value", version: 2, schema_type: "AVRO" },
        ];
      case "load_settings":
        // Preview fixture mirrors the backend policy: loopback host, distinct
        // ports and an absolute data directory.
        return { host: "127.0.0.1", kafka_port: 9092, http_port: 9094, data_dir: "/var/lib/streamline", log_level: "info" };
      case "start_server":
      case "stop_server":
        return null;
      case "produce_message":
        return null;
      case "consume_messages":
        return [
          { key: "user-1", value: '{"action":"click","page":"/home"}', partition: 0, offset: 0 },
          { key: "user-2", value: '{"action":"signup","email":"alice@example.com"}', partition: 1, offset: 0 },
          { key: "user-1", value: '{"action":"purchase","item":"widget"}', partition: 0, offset: 1 },
        ];
      case "create_topic":
        return null;
      case "delete_consumer_group":
        return null;
      case "describe_consumer_group":
        return {
          group_id: (args?.groupId as string) ?? "unknown",
          state: "Stable",
          protocol: "range",
          members: [
            { member_id: "member-1", client_id: "client-1", host: "/127.0.0.1", assignments: ["demo-events-0"] },
          ],
          offsets: [
            { topic: "demo-events", partition: 0, current_offset: 142, log_end_offset: 142, lag: 0 },
          ],
        };
      case "get_schema":
        return {
          subject: (args?.subject as string) ?? "unknown",
          version: 1,
          id: 1,
          schema_type: "JSON",
          schema: '{"type":"object","properties":{"action":{"type":"string"}}}',
          compatibility: "BACKWARD",
        };
      case "save_settings":
        return null;
      case "get_settings_warning":
      case "take_startup_error":
        return null;
      default:
        console.error(`[Streamline Desktop] Unhandled command in preview mode: "${cmd}"`);
        return null;
    }
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
