import { describe, it, expect } from "vitest";
import type {
  Tab,
  Toast,
  ServerStatus,
  TopicInfo,
  ConsumedMessage,
  ServerInfo,
  Settings,
  ServerSettingsPayload,
  ConsumerGroupInfo,
  ConsumerGroupDetail,
  GroupMember,
  GroupOffset,
  SchemaSubject,
  SchemaDetail,
} from "../types";
import { COLORS, IS_TAURI, invoke, inputStyle, btnStyle, thStyle, tdStyle } from "../types";

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

  it("should define ConsumedMessage with its partition", () => {
    const message: ConsumedMessage = {
      key: "key",
      value: "value",
      partition: 2,
      offset: 10,
    };
    expect(message.partition).toBe(2);
  });

  it("should define ServerInfo interface correctly", () => {
    const info: ServerInfo = {
      version: "0.2.0",
      uptime_secs: 3600,
      kafka_port: 9092,
      http_port: 9094,
    };
    expect(info.version).toBe("0.2.0");
  });

  it("should define Settings interface correctly", () => {
    const settings: Settings = {
      host: "127.0.0.1",
      kafkaPort: 9092,
      httpPort: 9094,
      dataDir: "/tmp/streamline",
      logLevel: "info",
    };
    expect(settings.kafkaPort).toBe(9092);
    expect(settings.host).toBe("127.0.0.1");
  });

  it("should define the persisted settings payload", () => {
    const settings: ServerSettingsPayload = {
      host: "127.0.0.1",
      kafka_port: 9092,
      http_port: 9094,
      data_dir: "/tmp/streamline",
      log_level: "info",
    };
    expect(settings.host).toBe("127.0.0.1");
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

describe("Preview Mode Invoke Mock", () => {
  it("IS_TAURI should be false in test environment", () => {
    expect(IS_TAURI).toBe(false);
  });

  it("invoke should be a function", () => {
    expect(typeof invoke).toBe("function");
  });

  it("get_server_status returns valid ServerStatus", async () => {
    const result = (await invoke("get_server_status")) as ServerStatus;
    expect(result).toBeDefined();
    expect(typeof result.running).toBe("boolean");
    expect(result.kafka_port).toBe(9092);
    expect(result.http_port).toBe(9094);
  });

  it("get_topics returns array of TopicInfo", async () => {
    const result = (await invoke("get_topics")) as TopicInfo[];
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);
    for (const topic of result) {
      expect(topic.name).toBeTruthy();
      expect(typeof topic.partitions).toBe("number");
      expect(typeof topic.messages).toBe("number");
    }
  });

  it("get_server_info returns valid ServerInfo", async () => {
    const result = (await invoke("get_server_info")) as ServerInfo;
    expect(result.version).toContain("0.2.0");
    expect(typeof result.uptime_secs).toBe("number");
  });

  it("list_consumer_groups returns array of ConsumerGroupInfo", async () => {
    const result = (await invoke("list_consumer_groups")) as ConsumerGroupInfo[];
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);
    for (const group of result) {
      expect(group.group_id).toBeTruthy();
      expect(group.state).toBeTruthy();
      expect(typeof group.members).toBe("number");
      expect(Array.isArray(group.topics)).toBe(true);
    }
  });

  it("list_schemas returns array of SchemaSubject", async () => {
    const result = (await invoke("list_schemas")) as SchemaSubject[];
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);
    for (const schema of result) {
      expect(schema.subject).toBeTruthy();
      expect(typeof schema.version).toBe("number");
      expect(schema.schema_type).toBeTruthy();
    }
  });

  it("load_settings returns valid Settings", async () => {
    const result = (await invoke("load_settings")) as Record<string, unknown>;
    expect(result.host).toBe("127.0.0.1");
    expect(result.kafka_port).toBe(9092);
    expect(result.http_port).toBe(9094);
    expect(result.data_dir).toBeTruthy();
    expect(result.log_level).toBeTruthy();
  });

  it("load_settings preview data satisfies the backend validation policy", async () => {
    const result = (await invoke("load_settings")) as Record<string, unknown>;
    expect(["127.0.0.1", "localhost", "::1"]).toContain(result.host);
    expect(result.kafka_port).not.toBe(result.http_port);
    expect(String(result.data_dir).startsWith("/")).toBe(true);
  });

  it("get_settings_warning returns null in preview mode", async () => {
    expect(await invoke("get_settings_warning")).toBeNull();
  });

  it("take_startup_error returns null in preview mode", async () => {
    expect(await invoke("take_startup_error")).toBeNull();
  });

  it("start_server returns null without error", async () => {
    const result = await invoke("start_server");
    expect(result).toBeNull();
  });

  it("stop_server returns null without error", async () => {
    const result = await invoke("stop_server");
    expect(result).toBeNull();
  });

  it("produce_message returns null without error", async () => {
    const result = await invoke("produce_message", {
      topic: "test",
      key: "k1",
      value: "v1",
    });
    expect(result).toBeNull();
  });

  it("consume_messages returns array of messages", async () => {
    const result = (await invoke("consume_messages", {
      topic: "test",
      limit: 50,
    })) as ConsumedMessage[];
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);
    for (const msg of result) {
      expect(typeof msg.key).toBe("string");
      expect(typeof msg.value).toBe("string");
      expect(typeof msg.partition).toBe("number");
      expect(typeof msg.offset).toBe("number");
    }
  });

  it("create_topic returns null without error", async () => {
    const result = await invoke("create_topic", {
      name: "new-topic",
      partitions: 3,
    });
    expect(result).toBeNull();
  });

  it("delete_consumer_group returns null without error", async () => {
    const result = await invoke("delete_consumer_group", {
      groupId: "test-group",
    });
    expect(result).toBeNull();
  });

  it("describe_consumer_group returns valid detail", async () => {
    const result = (await invoke("describe_consumer_group", {
      groupId: "my-group",
    })) as ConsumerGroupDetail;
    expect(result.group_id).toBe("my-group");
    expect(result.state).toBeTruthy();
    expect(result.protocol).toBeTruthy();
    expect(Array.isArray(result.members)).toBe(true);
    expect(result.members.length).toBeGreaterThan(0);
    expect(result.members[0].member_id).toBeTruthy();
    expect(Array.isArray(result.offsets)).toBe(true);
    expect(result.offsets.length).toBeGreaterThan(0);
    expect(typeof result.offsets[0].lag).toBe("number");
  });

  it("get_schema returns valid SchemaDetail", async () => {
    const result = (await invoke("get_schema", {
      subject: "events-value",
    })) as SchemaDetail;
    expect(result.subject).toBe("events-value");
    expect(typeof result.version).toBe("number");
    expect(typeof result.id).toBe("number");
    expect(result.schema_type).toBeTruthy();
    expect(result.schema).toBeTruthy();
    expect(result.compatibility).toBeTruthy();
  });

  it("save_settings returns null without error", async () => {
    const result = await invoke("save_settings", {
      settings: { kafka_port: 9092, http_port: 9094, data_dir: "./data", log_level: "debug" },
    });
    expect(result).toBeNull();
  });

  it("unknown command returns null", async () => {
    const result = await invoke("nonexistent_command");
    expect(result).toBeNull();
  });
});
