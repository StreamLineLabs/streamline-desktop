import { useCallback, useEffect, useState, useRef } from "react";

const IS_TAURI = !!(window as any).__TAURI__?.core?.invoke;

const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  (window as any).__TAURI__?.core?.invoke ??
  (async (cmd: string) => {
    console.warn(`[Streamline Desktop] Tauri not available — "${cmd}" returns mock data`);
    if (cmd === "get_server_status") return { running: false, kafka_port: 9092, http_port: 9094 };
    if (cmd === "get_topics") return [];
    if (cmd === "get_server_info") return { version: "0.2.0", uptime: 0, topics: 0, messages: 0 };
    return {};
  });

type Tab = "dashboard" | "topics" | "produce" | "consume" | "groups" | "schemas" | "settings";

interface Toast {
  id: number;
  message: string;
  type: "error" | "success" | "info";
}

interface ServerStatus {
  running: boolean;
  pid?: number;
  kafka_port: number;
  http_port: number;
}

interface TopicInfo {
  name: string;
  partitions: number;
  messages: number;
}

interface ServerInfo {
  version: string;
  uptime: number;
  topics: number;
  messages: number;
}

interface Settings {
  kafkaPort: number;
  httpPort: number;
  dataDir: string;
  logLevel: string;
}

interface ConsumerGroupInfo {
  group_id: string;
  state: string;
  members: number;
  topics: string[];
}

interface ConsumerGroupDetail {
  group_id: string;
  state: string;
  protocol: string;
  members: GroupMember[];
  offsets: GroupOffset[];
}

interface GroupMember {
  member_id: string;
  client_id: string;
  host: string;
  assignments: string[];
}

interface GroupOffset {
  topic: string;
  partition: number;
  current_offset: number;
  log_end_offset: number;
  lag: number;
}

interface SchemaSubject {
  subject: string;
  version: number;
  schema_type: string;
}

interface SchemaDetail {
  subject: string;
  version: number;
  id: number;
  schema_type: string;
  schema: string;
  compatibility: string;
}

const COLORS = {
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
};

export default function App() {
  const [tab, setTab] = useState<Tab>("dashboard");
  const [status, setStatus] = useState<ServerStatus>({ running: false, kafka_port: 9092, http_port: 9094 });
  const [topics, setTopics] = useState<TopicInfo[]>([]);
  const [serverInfo, setServerInfo] = useState<ServerInfo>({ version: "0.2.0", uptime: 0, topics: 0, messages: 0 });
  const [settings, setSettings] = useState<Settings>({ kafkaPort: 9092, httpPort: 9094, dataDir: "", logLevel: "info" });
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastIdRef = useRef(0);

  const showToast = useCallback((message: string, type: Toast["type"] = "error") => {
    const id = ++toastIdRef.current;
    setToasts((prev) => [...prev, { id, message, type }]);
    setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 5000);
  }, []);

  // Produce state
  const [produceTopic, setProduceTopic] = useState("");
  const [produceKey, setProduceKey] = useState("");
  const [produceValue, setProduceValue] = useState("");
  const [produceStatus, setProduceStatus] = useState<string | null>(null);

  // Consume state
  const [consumeTopic, setConsumeTopic] = useState("");
  const [consumeMessages, setConsumeMessages] = useState<Array<{ key: string; value: string; offset: number }>>([]);

  const refreshData = useCallback(async () => {
    try {
      const s = (await invoke("get_server_status")) as ServerStatus;
      setStatus(s);
      if (s.running) {
        const t = (await invoke("get_topics")) as TopicInfo[];
        setTopics(Array.isArray(t) ? t : []);
        const info = (await invoke("get_server_info")) as ServerInfo;
        setServerInfo(info);
      }
    } catch (e) {
      if (IS_TAURI) console.error("[Streamline Desktop] refresh failed:", e);
      showToast(`Refresh failed: ${e}`, "error");
    }
  }, []);

  useEffect(() => {
    refreshData();
    // Load persisted settings on startup
    invoke("load_settings").then((s: any) => {
      if (s) setSettings({ kafkaPort: s.kafka_port, httpPort: s.http_port, dataDir: s.data_dir, logLevel: s.log_level });
    }).catch((e: unknown) => {
      if (IS_TAURI) console.error("[Streamline Desktop] load_settings failed:", e);
    });
    const poll = setInterval(refreshData, 5000);
    return () => clearInterval(poll);
  }, [refreshData]);

  const handleStartStop = async () => {
    try {
      if (status.running) {
        await invoke("stop_server");
      } else {
        await invoke("start_server");
      }
      setTimeout(refreshData, 1000);
    } catch (e) {
      showToast(`Failed to ${status.running ? "stop" : "start"} server: ${e}`, "error");
    }
  };

  const handleProduce = async () => {
    if (!produceTopic || !produceValue) return;
    try {
      await invoke("produce_message", { topic: produceTopic, key: produceKey || null, value: produceValue });
      setProduceStatus("✓ Message sent");
      showToast("Message sent successfully", "success");
      setProduceValue("");
      setTimeout(() => setProduceStatus(null), 3000);
    } catch (e) {
      setProduceStatus(`✗ Failed: ${e}`);
      showToast(`Produce failed: ${e}`, "error");
    }
  };

  const handleConsume = async () => {
    if (!consumeTopic) return;
    try {
      const msgs = (await invoke("consume_messages", { topic: consumeTopic, limit: 50 })) as Array<{
        key: string;
        value: string;
        offset: number;
      }>;
      setConsumeMessages(Array.isArray(msgs) ? msgs : []);
    } catch (e) {
      showToast(`Consume failed: ${e}`, "error");
      setConsumeMessages([]);
    }
  };

  const formatUptime = (seconds: number) => {
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    return `${h}h ${m}m`;
  };

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: "dashboard", label: "Dashboard", icon: "📊" },
    { id: "topics", label: "Topics", icon: "📋" },
    { id: "produce", label: "Produce", icon: "📤" },
    { id: "consume", label: "Consume", icon: "📥" },
    { id: "groups", label: "Groups", icon: "👥" },
    { id: "schemas", label: "Schemas", icon: "📐" },
    { id: "settings", label: "Settings", icon: "⚙️" },
  ];

  return (
    <div style={{ display: "flex", height: "100vh", fontFamily: "system-ui, -apple-system, sans-serif", position: "relative" }}>
      {/* Toast notifications */}
      <div style={{ position: "fixed", top: 16, right: 16, zIndex: 9999, display: "flex", flexDirection: "column", gap: 8 }}>
        {toasts.map((t) => (
          <div
            key={t.id}
            style={{
              padding: "12px 20px",
              borderRadius: 8,
              color: "#fff",
              fontSize: 13,
              fontWeight: 500,
              maxWidth: 360,
              boxShadow: "0 4px 12px rgba(0,0,0,0.3)",
              background: t.type === "error" ? COLORS.red : t.type === "success" ? COLORS.green : COLORS.blue,
              cursor: "pointer",
              animation: "fadeIn 0.2s ease-out",
            }}
            onClick={() => setToasts((prev) => prev.filter((x) => x.id !== t.id))}
          >
            {t.type === "error" ? "✗ " : t.type === "success" ? "✓ " : "ℹ "}{t.message}
          </div>
        ))}
      </div>

      {/* Sidebar */}
      <aside style={{ width: 220, background: COLORS.sidebar, color: COLORS.text, display: "flex", flexDirection: "column", padding: "16px 0", borderRight: `1px solid ${COLORS.border}` }}>
        <div style={{ padding: "0 16px 24px", fontSize: 18, fontWeight: 700 }}>⚡ Streamline</div>

        <div style={{ padding: "0 16px 16px", fontSize: 13, display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ width: 10, height: 10, borderRadius: "50%", background: status.running ? COLORS.green : COLORS.red, display: "inline-block" }} />
          {status.running ? "Running" : "Stopped"}
        </div>

        <button
          onClick={handleStartStop}
          style={{
            margin: "0 16px 16px", padding: "8px 16px", borderRadius: 6,
            background: status.running ? COLORS.red : COLORS.green,
            color: "#fff", border: "none", cursor: "pointer", fontSize: 13, fontWeight: 600,
          }}
        >
          {status.running ? "Stop Server" : "Start Server"}
        </button>

        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            style={{
              background: tab === t.id ? COLORS.active : "transparent",
              color: COLORS.text, border: "none", textAlign: "left",
              padding: "10px 16px", cursor: "pointer", fontSize: 14,
              borderLeft: tab === t.id ? `3px solid ${COLORS.blue}` : "3px solid transparent",
            }}
          >
            {t.icon} {t.label}
          </button>
        ))}

        <div style={{ flex: 1 }} />
        <div style={{ padding: "8px 16px", fontSize: 11, opacity: 0.5 }}>
          v{serverInfo.version} · Port {status.kafka_port}
        </div>
      </aside>

      {/* Main content */}
      <main style={{ flex: 1, background: COLORS.bg, color: COLORS.text, overflow: "auto", padding: 32 }}>
        {tab === "dashboard" && <DashboardTab status={status} info={serverInfo} topics={topics} formatUptime={formatUptime} />}
        {tab === "topics" && <TopicsTab topics={topics} onRefresh={refreshData} />}
        {tab === "produce" && (
          <ProduceTab
            topic={produceTopic} setTopic={setProduceTopic}
            msgKey={produceKey} setKey={setProduceKey}
            value={produceValue} setValue={setProduceValue}
            onSend={handleProduce} status={produceStatus}
            topics={topics}
          />
        )}
        {tab === "consume" && (
          <ConsumeTab
            topic={consumeTopic} setTopic={setConsumeTopic}
            messages={consumeMessages} onFetch={handleConsume}
            topics={topics}
          />
        )}
        {tab === "groups" && <ConsumerGroupsTab />}
        {tab === "schemas" && <SchemasTab />}
        {tab === "settings" && <SettingsTab settings={settings} setSettings={setSettings} />}
      </main>
    </div>
  );
}

// -- Dashboard Tab --

function DashboardTab({ status, info, topics, formatUptime }: {
  status: ServerStatus; info: ServerInfo; topics: TopicInfo[]; formatUptime: (s: number) => string;
}) {
  const totalMessages = topics.reduce((sum, t) => sum + t.messages, 0);
  const cards = [
    { label: "Status", value: status.running ? "Running" : "Stopped", color: status.running ? COLORS.green : COLORS.red },
    { label: "Uptime", value: formatUptime(info.uptime), color: COLORS.blue },
    { label: "Topics", value: String(topics.length), color: COLORS.purple },
    { label: "Messages", value: totalMessages.toLocaleString(), color: COLORS.yellow },
  ];

  return (
    <div>
      <h2 style={{ marginTop: 0, fontSize: 24, fontWeight: 600 }}>Dashboard</h2>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: 16, marginBottom: 32 }}>
        {cards.map((c) => (
          <div key={c.label} style={{ background: COLORS.card, borderRadius: 8, padding: 20, borderLeft: `4px solid ${c.color}` }}>
            <div style={{ fontSize: 12, color: COLORS.textDim, marginBottom: 4 }}>{c.label}</div>
            <div style={{ fontSize: 24, fontWeight: 700 }}>{c.value}</div>
          </div>
        ))}
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
        <div style={{ background: COLORS.card, borderRadius: 8, padding: 20 }}>
          <h3 style={{ marginTop: 0, fontSize: 16 }}>Connection Details</h3>
          <InfoRow label="Kafka Port" value={String(status.kafka_port)} />
          <InfoRow label="HTTP Port" value={String(status.http_port)} />
          <InfoRow label="Version" value={info.version} />
          {status.pid && <InfoRow label="PID" value={String(status.pid)} />}
        </div>

        <div style={{ background: COLORS.card, borderRadius: 8, padding: 20 }}>
          <h3 style={{ marginTop: 0, fontSize: 16 }}>Top Topics</h3>
          {topics.length === 0 ? (
            <div style={{ color: COLORS.textDim, fontSize: 13 }}>No topics yet</div>
          ) : (
            topics.slice(0, 5).map((t) => (
              <div key={t.name} style={{ display: "flex", justifyContent: "space-between", padding: "4px 0", fontSize: 13 }}>
                <span>{t.name}</span>
                <span style={{ color: COLORS.textDim }}>{t.messages.toLocaleString()} msgs</span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

// -- Topics Tab --

function TopicsTab({ topics, onRefresh }: { topics: TopicInfo[]; onRefresh: () => void }) {
  const [newTopicName, setNewTopicName] = useState("");
  const [newTopicPartitions, setNewTopicPartitions] = useState(1);

  const handleCreate = async () => {
    if (!newTopicName) return;
    try {
      await invoke("create_topic", { name: newTopicName, partitions: newTopicPartitions });
      setNewTopicName("");
      setNewTopicPartitions(1);
      onRefresh();
    } catch (e) {
      console.error("[Streamline Desktop] create topic failed:", e);
    }
  };

  return (
    <div>
      <h2 style={{ marginTop: 0, fontSize: 24, fontWeight: 600 }}>Topics</h2>

      <div style={{ background: COLORS.card, borderRadius: 8, padding: 20, marginBottom: 24 }}>
        <h3 style={{ marginTop: 0, fontSize: 16 }}>Create Topic</h3>
        <div style={{ display: "flex", gap: 12, alignItems: "flex-end" }}>
          <FieldLabel text="Name">
            <Input value={newTopicName} onChange={setNewTopicName} placeholder="my-topic" />
          </FieldLabel>
          <FieldLabel text="Partitions">
            <Input type="number" value={String(newTopicPartitions)} onChange={(v) => setNewTopicPartitions(+v)} />
          </FieldLabel>
          <button onClick={handleCreate} style={btnStyle}>Create</button>
        </div>
      </div>

      <div style={{ background: COLORS.card, borderRadius: 8, overflow: "hidden" }}>
        <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 14 }}>
          <thead>
            <tr style={{ borderBottom: `1px solid ${COLORS.border}`, textAlign: "left" }}>
              <th style={thStyle}>Name</th>
              <th style={thStyle}>Partitions</th>
              <th style={thStyle}>Messages</th>
            </tr>
          </thead>
          <tbody>
            {topics.length === 0 ? (
              <tr><td colSpan={3} style={{ ...tdStyle, color: COLORS.textDim, textAlign: "center" }}>No topics</td></tr>
            ) : (
              topics.map((t) => (
                <tr key={t.name} style={{ borderBottom: `1px solid ${COLORS.border}` }}>
                  <td style={tdStyle}>{t.name}</td>
                  <td style={tdStyle}>{t.partitions}</td>
                  <td style={tdStyle}>{t.messages.toLocaleString()}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// -- Produce Tab --

function ProduceTab({ topic, setTopic, msgKey, setKey, value, setValue, onSend, status, topics }: {
  topic: string; setTopic: (v: string) => void;
  msgKey: string; setKey: (v: string) => void;
  value: string; setValue: (v: string) => void;
  onSend: () => void; status: string | null;
  topics: TopicInfo[];
}) {
  return (
    <div>
      <h2 style={{ marginTop: 0, fontSize: 24, fontWeight: 600 }}>Produce Message</h2>
      <div style={{ background: COLORS.card, borderRadius: 8, padding: 24, maxWidth: 600 }}>
        <FieldLabel text="Topic">
          <select
            value={topic}
            onChange={(e) => setTopic(e.target.value)}
            style={{ ...inputStyle, width: "100%", appearance: "auto" }}
          >
            <option value="">Select topic...</option>
            {topics.map((t) => <option key={t.name} value={t.name}>{t.name}</option>)}
          </select>
        </FieldLabel>
        <FieldLabel text="Key (optional)">
          <Input value={msgKey} onChange={setKey} placeholder="message-key" />
        </FieldLabel>
        <FieldLabel text="Value">
          <textarea
            value={value}
            onChange={(e) => setValue(e.target.value)}
            placeholder='{"event": "user_signup", "user_id": "123"}'
            rows={6}
            style={{ ...inputStyle, width: "100%", resize: "vertical", fontFamily: "monospace", fontSize: 13, boxSizing: "border-box" }}
          />
        </FieldLabel>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button onClick={onSend} style={btnStyle} disabled={!topic || !value}>Send Message</button>
          {status && <span style={{ fontSize: 13, color: status.startsWith("✓") ? COLORS.green : COLORS.red }}>{status}</span>}
        </div>
      </div>
    </div>
  );
}

// -- Consume Tab --

function ConsumeTab({ topic, setTopic, messages, onFetch, topics }: {
  topic: string; setTopic: (v: string) => void;
  messages: Array<{ key: string; value: string; offset: number }>;
  onFetch: () => void; topics: TopicInfo[];
}) {
  const [expandedRow, setExpandedRow] = useState<number | null>(null);

  const formatValue = (value: string): { formatted: string; isJson: boolean } => {
    try {
      const parsed = JSON.parse(value);
      return { formatted: JSON.stringify(parsed, null, 2), isJson: true };
    } catch {
      return { formatted: value, isJson: false };
    }
  };

  return (
    <div>
      <h2 style={{ marginTop: 0, fontSize: 24, fontWeight: 600 }}>Consume Messages</h2>
      <div style={{ background: COLORS.card, borderRadius: 8, padding: 24, marginBottom: 24 }}>
        <div style={{ display: "flex", gap: 12, alignItems: "flex-end" }}>
          <FieldLabel text="Topic">
            <select
              value={topic}
              onChange={(e) => setTopic(e.target.value)}
              style={{ ...inputStyle, width: 250, appearance: "auto" }}
            >
              <option value="">Select topic...</option>
              {topics.map((t) => <option key={t.name} value={t.name}>{t.name}</option>)}
            </select>
          </FieldLabel>
          <button onClick={onFetch} style={btnStyle} disabled={!topic}>Fetch Messages</button>
        </div>
      </div>

      <div style={{ background: COLORS.card, borderRadius: 8, overflow: "hidden" }}>
        <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13, fontFamily: "monospace" }}>
          <thead>
            <tr style={{ borderBottom: `1px solid ${COLORS.border}`, textAlign: "left" }}>
              <th style={thStyle}>Offset</th>
              <th style={thStyle}>Key</th>
              <th style={thStyle}>Value</th>
            </tr>
          </thead>
          <tbody>
            {messages.length === 0 ? (
              <tr><td colSpan={3} style={{ ...tdStyle, color: COLORS.textDim, textAlign: "center" }}>No messages</td></tr>
            ) : (
              messages.map((m, i) => {
                const { formatted, isJson } = formatValue(m.value);
                const isExpanded = expandedRow === i;
                return (
                  <tr
                    key={i}
                    style={{ borderBottom: `1px solid ${COLORS.border}`, cursor: isJson ? "pointer" : "default", verticalAlign: "top" }}
                    onClick={() => isJson && setExpandedRow(isExpanded ? null : i)}
                  >
                    <td style={{ ...tdStyle, width: 80 }}>{m.offset}</td>
                    <td style={{ ...tdStyle, width: 150, color: COLORS.textDim }}>{m.key || "—"}</td>
                    <td style={{ ...tdStyle, maxWidth: 500 }}>
                      {isExpanded ? (
                        <pre style={{ margin: 0, whiteSpace: "pre-wrap", wordBreak: "break-word", color: COLORS.green, fontSize: 12 }}>
                          {formatted}
                        </pre>
                      ) : (
                        <div style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", display: "flex", alignItems: "center", gap: 6 }}>
                          {isJson && <span style={{ color: COLORS.blue, fontSize: 10 }}>JSON</span>}
                          {m.value}
                        </div>
                      )}
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// -- Consumer Groups Tab --

function ConsumerGroupsTab() {
  const [groups, setGroups] = useState<ConsumerGroupInfo[]>([]);
  const [selectedGroup, setSelectedGroup] = useState<ConsumerGroupDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchGroups = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = (await invoke("list_consumer_groups")) as ConsumerGroupInfo[];
      setGroups(Array.isArray(result) ? result : []);
    } catch (e) {
      setError(String(e));
      setGroups([]);
    } finally {
      setLoading(false);
    }
  };

  const describeGroup = async (groupId: string) => {
    try {
      const detail = (await invoke("describe_consumer_group", { groupId })) as ConsumerGroupDetail;
      setSelectedGroup(detail);
    } catch (e) {
      setError(`Failed to describe group: ${e}`);
    }
  };

  const deleteGroup = async (groupId: string) => {
    try {
      await invoke("delete_consumer_group", { groupId });
      setSelectedGroup(null);
      fetchGroups();
    } catch (e) {
      setError(`Failed to delete group: ${e}`);
    }
  };

  useEffect(() => { fetchGroups(); }, []);

  const stateColor = (state: string) => {
    switch (state.toLowerCase()) {
      case "stable": return COLORS.green;
      case "preparing_rebalance":
      case "completing_rebalance": return COLORS.yellow;
      case "empty": return COLORS.textDim;
      case "dead": return COLORS.red;
      default: return COLORS.textDim;
    }
  };

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", gap: 16, marginBottom: 24 }}>
        <h2 style={{ marginTop: 0, marginBottom: 0, fontSize: 24, fontWeight: 600 }}>Consumer Groups</h2>
        <button onClick={fetchGroups} style={{ ...btnStyle, padding: "6px 14px", fontSize: 13 }}>
          {loading ? "Loading…" : "Refresh"}
        </button>
      </div>

      {error && (
        <div style={{ background: "#3e1a1a", borderRadius: 8, padding: 12, marginBottom: 16, color: COLORS.red, fontSize: 13 }}>
          {error}
        </div>
      )}

      <div style={{ display: "grid", gridTemplateColumns: selectedGroup ? "1fr 1fr" : "1fr", gap: 16 }}>
        {/* Groups list */}
        <div style={{ background: COLORS.card, borderRadius: 8, overflow: "hidden" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 14 }}>
            <thead>
              <tr style={{ borderBottom: `1px solid ${COLORS.border}`, textAlign: "left" }}>
                <th style={thStyle}>Group ID</th>
                <th style={thStyle}>State</th>
                <th style={thStyle}>Members</th>
              </tr>
            </thead>
            <tbody>
              {groups.length === 0 ? (
                <tr><td colSpan={3} style={{ ...tdStyle, color: COLORS.textDim, textAlign: "center" }}>No consumer groups</td></tr>
              ) : (
                groups.map((g: ConsumerGroupInfo) => (
                  <tr
                    key={g.group_id}
                    onClick={() => describeGroup(g.group_id)}
                    style={{
                      borderBottom: `1px solid ${COLORS.border}`,
                      cursor: "pointer",
                      background: selectedGroup?.group_id === g.group_id ? COLORS.active : "transparent",
                    }}
                  >
                    <td style={tdStyle}>{g.group_id}</td>
                    <td style={tdStyle}>
                      <span style={{ color: stateColor(g.state), fontWeight: 600, fontSize: 12 }}>{g.state}</span>
                    </td>
                    <td style={tdStyle}>{g.members}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* Group detail panel */}
        {selectedGroup && (
          <div style={{ background: COLORS.card, borderRadius: 8, padding: 20 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
              <h3 style={{ margin: 0, fontSize: 16 }}>{selectedGroup.group_id}</h3>
              <div style={{ display: "flex", gap: 8 }}>
                <button
                  onClick={() => deleteGroup(selectedGroup.group_id)}
                  style={{ ...btnStyle, background: COLORS.red, padding: "4px 12px", fontSize: 12 }}
                >
                  Delete
                </button>
                <button
                  onClick={() => setSelectedGroup(null)}
                  style={{ ...btnStyle, background: "transparent", border: `1px solid ${COLORS.border}`, padding: "4px 12px", fontSize: 12 }}
                >
                  Close
                </button>
              </div>
            </div>

            <InfoRow label="State" value={selectedGroup.state} />
            <InfoRow label="Protocol" value={selectedGroup.protocol || "—"} />
            <InfoRow label="Members" value={String(selectedGroup.members?.length ?? 0)} />

            {selectedGroup.members && selectedGroup.members.length > 0 && (
              <div style={{ marginTop: 16 }}>
                <h4 style={{ margin: "0 0 8px", fontSize: 13, color: COLORS.textDim }}>Members</h4>
                {selectedGroup.members.map((m: GroupMember) => (
                  <div key={m.member_id} style={{ background: COLORS.bg, borderRadius: 6, padding: 10, marginBottom: 8, fontSize: 12 }}>
                    <div style={{ fontWeight: 600 }}>{m.client_id}</div>
                    <div style={{ color: COLORS.textDim, marginTop: 2 }}>{m.host}</div>
                    {m.assignments && m.assignments.length > 0 && (
                      <div style={{ marginTop: 4, color: COLORS.blue }}>
                        {m.assignments.join(", ")}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}

            {selectedGroup.offsets && selectedGroup.offsets.length > 0 && (
              <div style={{ marginTop: 16 }}>
                <h4 style={{ margin: "0 0 8px", fontSize: 13, color: COLORS.textDim }}>Partition Offsets</h4>
                <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12, fontFamily: "monospace" }}>
                  <thead>
                    <tr style={{ borderBottom: `1px solid ${COLORS.border}` }}>
                      <th style={{ ...thStyle, padding: "6px 8px" }}>Topic</th>
                      <th style={{ ...thStyle, padding: "6px 8px" }}>Part</th>
                      <th style={{ ...thStyle, padding: "6px 8px" }}>Offset</th>
                      <th style={{ ...thStyle, padding: "6px 8px" }}>End</th>
                      <th style={{ ...thStyle, padding: "6px 8px" }}>Lag</th>
                    </tr>
                  </thead>
                  <tbody>
                    {selectedGroup.offsets.map((o: GroupOffset, i: number) => (
                      <tr key={i} style={{ borderBottom: `1px solid ${COLORS.border}` }}>
                        <td style={{ padding: "4px 8px" }}>{o.topic}</td>
                        <td style={{ padding: "4px 8px" }}>{o.partition}</td>
                        <td style={{ padding: "4px 8px" }}>{o.current_offset}</td>
                        <td style={{ padding: "4px 8px" }}>{o.log_end_offset}</td>
                        <td style={{ padding: "4px 8px", color: o.lag > 0 ? COLORS.yellow : COLORS.green, fontWeight: 600 }}>
                          {o.lag}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// -- Schemas Tab --

function SchemasTab() {
  const [subjects, setSubjects] = useState<SchemaSubject[]>([]);
  const [selectedSchema, setSelectedSchema] = useState<SchemaDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchSubjects = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = (await invoke("list_schemas")) as SchemaSubject[];
      setSubjects(Array.isArray(result) ? result : []);
    } catch (e) {
      setError(String(e));
      setSubjects([]);
    } finally {
      setLoading(false);
    }
  };

  const viewSchema = async (subject: string) => {
    try {
      const detail = (await invoke("get_schema", { subject })) as SchemaDetail;
      setSelectedSchema(detail);
    } catch (e) {
      setError(`Failed to load schema: ${e}`);
    }
  };

  useEffect(() => { fetchSubjects(); }, []);

  const formatSchema = (schema: string): string => {
    try {
      return JSON.stringify(JSON.parse(schema), null, 2);
    } catch {
      return schema;
    }
  };

  const typeColor = (schemaType: string) => {
    switch (schemaType?.toUpperCase()) {
      case "AVRO": return COLORS.green;
      case "PROTOBUF": return COLORS.blue;
      case "JSON": return COLORS.yellow;
      default: return COLORS.textDim;
    }
  };

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", gap: 16, marginBottom: 24 }}>
        <h2 style={{ marginTop: 0, marginBottom: 0, fontSize: 24, fontWeight: 600 }}>Schema Registry</h2>
        <button onClick={fetchSubjects} style={{ ...btnStyle, padding: "6px 14px", fontSize: 13 }}>
          {loading ? "Loading…" : "Refresh"}
        </button>
      </div>

      {error && (
        <div style={{ background: "#3e1a1a", borderRadius: 8, padding: 12, marginBottom: 16, color: COLORS.red, fontSize: 13 }}>
          {error}
        </div>
      )}

      <div style={{ display: "grid", gridTemplateColumns: selectedSchema ? "350px 1fr" : "1fr", gap: 16 }}>
        {/* Subjects list */}
        <div style={{ background: COLORS.card, borderRadius: 8, overflow: "hidden" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 14 }}>
            <thead>
              <tr style={{ borderBottom: `1px solid ${COLORS.border}`, textAlign: "left" }}>
                <th style={thStyle}>Subject</th>
                <th style={thStyle}>Type</th>
              </tr>
            </thead>
            <tbody>
              {subjects.length === 0 ? (
                <tr><td colSpan={2} style={{ ...tdStyle, color: COLORS.textDim, textAlign: "center" }}>No schemas registered</td></tr>
              ) : (
                subjects.map((s: SchemaSubject) => (
                  <tr
                    key={s.subject}
                    onClick={() => viewSchema(s.subject)}
                    style={{
                      borderBottom: `1px solid ${COLORS.border}`,
                      cursor: "pointer",
                      background: selectedSchema?.subject === s.subject ? COLORS.active : "transparent",
                    }}
                  >
                    <td style={tdStyle}>{s.subject}</td>
                    <td style={tdStyle}>
                      {s.schema_type && (
                        <span style={{ color: typeColor(s.schema_type), fontWeight: 600, fontSize: 12 }}>
                          {s.schema_type}
                        </span>
                      )}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* Schema detail panel */}
        {selectedSchema && (
          <div style={{ background: COLORS.card, borderRadius: 8, padding: 20 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
              <h3 style={{ margin: 0, fontSize: 16 }}>{selectedSchema.subject}</h3>
              <button
                onClick={() => setSelectedSchema(null)}
                style={{ ...btnStyle, background: "transparent", border: `1px solid ${COLORS.border}`, padding: "4px 12px", fontSize: 12 }}
              >
                Close
              </button>
            </div>

            <InfoRow label="Schema ID" value={String(selectedSchema.id)} />
            <InfoRow label="Version" value={String(selectedSchema.version)} />
            <InfoRow label="Type" value={selectedSchema.schema_type || "—"} />
            {selectedSchema.compatibility && <InfoRow label="Compatibility" value={selectedSchema.compatibility} />}

            <div style={{ marginTop: 16 }}>
              <div style={{ fontSize: 13, color: COLORS.textDim, marginBottom: 8 }}>Schema Definition</div>
              <pre style={{
                background: COLORS.bg, borderRadius: 8, padding: 16,
                fontSize: 12, fontFamily: "monospace", overflow: "auto",
                maxHeight: 400, margin: 0, whiteSpace: "pre-wrap", wordBreak: "break-word",
                color: COLORS.green, border: `1px solid ${COLORS.border}`,
              }}>
                {formatSchema(selectedSchema.schema)}
              </pre>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// -- Settings Tab --

function SettingsTab({ settings, setSettings }: { settings: Settings; setSettings: (s: Settings) => void }) {
  const [saved, setSaved] = useState(false);

  const handleSave = async () => {
    try {
      await invoke("save_settings", {
        settings: {
          kafka_port: settings.kafkaPort,
          http_port: settings.httpPort,
          data_dir: settings.dataDir,
          log_level: settings.logLevel,
        },
      });
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (e) {
      console.error("[Streamline Desktop] save settings failed:", e);
    }
  };

  return (
    <div style={{ maxWidth: 480 }}>
      <h2 style={{ marginTop: 0, fontSize: 24, fontWeight: 600 }}>Server Settings</h2>
      <div style={{ background: COLORS.card, borderRadius: 8, padding: 24 }}>
        <FieldLabel text="Kafka Port">
          <Input type="number" value={String(settings.kafkaPort)} onChange={(v) => setSettings({ ...settings, kafkaPort: +v })} />
        </FieldLabel>
        <FieldLabel text="HTTP Port">
          <Input type="number" value={String(settings.httpPort)} onChange={(v) => setSettings({ ...settings, httpPort: +v })} />
        </FieldLabel>
        <FieldLabel text="Data Directory">
          <Input value={settings.dataDir} onChange={(v) => setSettings({ ...settings, dataDir: v })} placeholder="/var/lib/streamline" />
        </FieldLabel>
        <FieldLabel text="Log Level">
          <select
            value={settings.logLevel}
            onChange={(e) => setSettings({ ...settings, logLevel: e.target.value })}
            style={{ ...inputStyle, width: "100%", appearance: "auto" }}
          >
            {["trace", "debug", "info", "warn", "error"].map((l) => <option key={l}>{l}</option>)}
          </select>
        </FieldLabel>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button onClick={handleSave} style={btnStyle}>Save Settings</button>
          {saved && <span style={{ fontSize: 13, color: COLORS.green }}>✓ Settings saved</span>}
        </div>
        <p style={{ fontSize: 12, opacity: 0.6, marginBottom: 0 }}>Changes take effect after restarting the server.</p>
      </div>
    </div>
  );
}

// -- Shared Components --

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "6px 0", fontSize: 13 }}>
      <span style={{ color: COLORS.textDim }}>{label}</span>
      <span style={{ fontFamily: "monospace" }}>{value}</span>
    </div>
  );
}

function FieldLabel({ text, children }: { text: string; children: React.ReactNode }) {
  return (
    <label style={{ display: "block", marginBottom: 16 }}>
      <div style={{ marginBottom: 4, fontSize: 13, color: COLORS.textDim }}>{text}</div>
      {children}
    </label>
  );
}

function Input({ value, onChange, placeholder, type }: {
  value: string; onChange: (v: string) => void; placeholder?: string; type?: string;
}) {
  return (
    <input
      type={type || "text"}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      style={inputStyle}
    />
  );
}

const inputStyle: React.CSSProperties = {
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

const btnStyle: React.CSSProperties = {
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

const thStyle: React.CSSProperties = { padding: "12px 16px", fontSize: 12, color: COLORS.textDim, fontWeight: 600, textTransform: "uppercase" };
const tdStyle: React.CSSProperties = { padding: "10px 16px" };
