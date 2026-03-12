import { describe, it, expect } from "vitest";
import type {
  Tab,
  Toast,
  ServerStatus,
  TopicInfo,
  ServerInfo,
  Settings,
  ConsumerGroupInfo,
  ConsumerGroupDetail,
  GroupMember,
  GroupOffset,
  SchemaSubject,
  SchemaDetail,
} from "../types";
import { COLORS, inputStyle, btnStyle, thStyle, tdStyle } from "../types";

describe("Desktop Types", () => {
  it("should satisfy Tab type constraint", () => {
    const validTabs: Tab[] = [
      "dashboard",
      "topics",
      "produce",
      "consume",
      "groups",
      "schemas",
      "settings",
    ];
    expect(validTabs).toHaveLength(7);
  });

  it("should define Toast interface correctly", () => {
    const toast: Toast = { id: 1, message: "hello", type: "success" };
    expect(toast.id).toBe(1);
    expect(toast.type).toBe("success");
  });

  it("should define ServerStatus interface correctly", () => {
    const status: ServerStatus = {
      running: true,
      pid: 1234,
      kafka_port: 9092,
      http_port: 9094,
    };
    expect(status.running).toBe(true);
    expect(status.kafka_port).toBe(9092);
  });

  it("should define TopicInfo interface correctly", () => {
    const topic: TopicInfo = { name: "events", partitions: 3, messages: 100 };
    expect(topic.name).toBe("events");
    expect(topic.partitions).toBe(3);
  });

  it("should define ServerInfo interface correctly", () => {
    const info: ServerInfo = {
      version: "0.2.0",
      uptime: 3600,
      topics: 5,
      messages: 1000,
    };
    expect(info.version).toBe("0.2.0");
  });

  it("should define Settings interface correctly", () => {
    const settings: Settings = {
      kafkaPort: 9092,
      httpPort: 9094,
      dataDir: "/tmp/streamline",
      logLevel: "info",
    };
    expect(settings.kafkaPort).toBe(9092);
  });

  it("should define ConsumerGroupInfo interface correctly", () => {
    const group: ConsumerGroupInfo = {
      group_id: "my-group",
      state: "Stable",
      members: 3,
      topics: ["events", "logs"],
    };
    expect(group.topics).toHaveLength(2);
  });

  it("should define ConsumerGroupDetail with members and offsets", () => {
    const member: GroupMember = {
      member_id: "m1",
      client_id: "c1",
      host: "localhost",
      assignments: ["events-0"],
    };
    const offset: GroupOffset = {
      topic: "events",
      partition: 0,
      current_offset: 100,
      log_end_offset: 150,
      lag: 50,
    };
    const detail: ConsumerGroupDetail = {
      group_id: "my-group",
      state: "Stable",
      protocol: "range",
      members: [member],
      offsets: [offset],
    };
    expect(detail.members).toHaveLength(1);
    expect(detail.offsets[0].lag).toBe(50);
  });

  it("should define SchemaSubject and SchemaDetail correctly", () => {
    const subject: SchemaSubject = {
      subject: "events-value",
      version: 1,
      schema_type: "AVRO",
    };
    expect(subject.schema_type).toBe("AVRO");

    const detail: SchemaDetail = {
      subject: "events-value",
      version: 1,
      id: 42,
      schema_type: "AVRO",
      schema: '{"type": "record"}',
      compatibility: "BACKWARD",
    };
    expect(detail.id).toBe(42);
  });
});

describe("Desktop Constants", () => {
  it("should define all expected COLORS keys", () => {
    const expectedKeys = [
      "bg",
      "sidebar",
      "active",
      "card",
      "border",
      "text",
      "textDim",
      "green",
      "red",
      "blue",
      "yellow",
      "purple",
    ];
    for (const key of expectedKeys) {
      expect(COLORS).toHaveProperty(key);
    }
  });

  it("should define COLORS as hex strings", () => {
    for (const value of Object.values(COLORS)) {
      expect(value).toMatch(/^#[0-9a-f]{3,6}$/);
    }
  });

  it("should define inputStyle with expected properties", () => {
    expect(inputStyle.borderRadius).toBe(6);
    expect(inputStyle.fontSize).toBe(14);
    expect(inputStyle.boxSizing).toBe("border-box");
  });

  it("should define btnStyle with expected properties", () => {
    expect(btnStyle.borderRadius).toBe(6);
    expect(btnStyle.fontWeight).toBe(600);
    expect(btnStyle.border).toBe("none");
  });

  it("should define thStyle with uppercase text transform", () => {
    expect(thStyle.textTransform).toBe("uppercase");
    expect(thStyle.fontWeight).toBe(600);
  });

  it("should define tdStyle with padding", () => {
    expect(tdStyle.padding).toBe("10px 16px");
  });
});
