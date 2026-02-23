import { useEffect, useState } from "react";

// Tauri command invoke helper (falls back to no-op outside Tauri)
const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> =
  (window as any).__TAURI__?.core?.invoke ??
  (async () => ({ running: false, kafka_port: 9092, http_port: 9094 }));

type Tab = "dashboard" | "topics" | "produce" | "consume" | "streamql" | "settings";

interface ServerStatus {
  running: boolean;
  pid?: number;
  kafka_port: number;
  http_port: number;
}

interface Settings {
  kafkaPort: number;
  httpPort: number;
  dataDir: string;
  logLevel: string;
}

export default function App() {
  const [tab, setTab] = useState<Tab>("dashboard");
  const [status, setStatus] = useState<ServerStatus>({
    running: false,
    kafka_port: 9092,
    http_port: 9094,
  });
  const [settings, setSettings] = useState<Settings>({
    kafkaPort: 9092,
    httpPort: 9094,
    dataDir: "",
    logLevel: "info",
  });

  useEffect(() => {
    const poll = setInterval(async () => {
      try {
        const s = (await invoke("get_server_status")) as ServerStatus;
        setStatus(s);
      } catch {
        /* outside Tauri */
      }
    }, 3000);
    return () => clearInterval(poll);
  }, []);

  const dashboardUrl = `http://127.0.0.1:${status.http_port}`;

  const tabs: { id: Tab; label: string }[] = [
    { id: "dashboard", label: "Dashboard" },
    { id: "topics", label: "Topics" },
    { id: "produce", label: "Produce" },
    { id: "consume", label: "Consume" },
    { id: "streamql", label: "StreamQL" },
    { id: "settings", label: "Settings" },
  ];

  return (
    <div style={{ display: "flex", height: "100vh", fontFamily: "system-ui, sans-serif" }}>
      {/* Sidebar */}
      <aside
        style={{
          width: 200,
          background: "#1a1a2e",
          color: "#eee",
          display: "flex",
          flexDirection: "column",
          padding: "16px 0",
        }}
      >
        <div style={{ padding: "0 16px 24px", fontSize: 18, fontWeight: 700 }}>
          ⚡ Streamline
        </div>

        {/* Connection indicator */}
        <div style={{ padding: "0 16px 16px", fontSize: 13, display: "flex", alignItems: "center", gap: 8 }}>
          <span
            style={{
              width: 10,
              height: 10,
              borderRadius: "50%",
              background: status.running ? "#4caf50" : "#f44336",
              display: "inline-block",
            }}
          />
          {status.running ? "Connected" : "Disconnected"}
        </div>

        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            style={{
              background: tab === t.id ? "#16213e" : "transparent",
              color: "#eee",
              border: "none",
              textAlign: "left",
              padding: "10px 16px",
              cursor: "pointer",
              fontSize: 14,
            }}
          >
            {t.label}
          </button>
        ))}

        <div style={{ flex: 1 }} />
        <div style={{ padding: "8px 16px", fontSize: 11, opacity: 0.5 }}>v0.2.0</div>
      </aside>

      {/* Main content */}
      <main style={{ flex: 1, background: "#0f0f23", color: "#eee", overflow: "auto" }}>
        {tab === "settings" ? (
          <div style={{ padding: 32, maxWidth: 480 }}>
            <h2 style={{ marginTop: 0 }}>Server Settings</h2>
            <Label text="Kafka Port">
              <input
                type="number"
                value={settings.kafkaPort}
                onChange={(e) => setSettings({ ...settings, kafkaPort: +e.target.value })}
              />
            </Label>
            <Label text="HTTP Port">
              <input
                type="number"
                value={settings.httpPort}
                onChange={(e) => setSettings({ ...settings, httpPort: +e.target.value })}
              />
            </Label>
            <Label text="Data Directory">
              <input
                value={settings.dataDir}
                onChange={(e) => setSettings({ ...settings, dataDir: e.target.value })}
                placeholder="/var/lib/streamline"
              />
            </Label>
            <Label text="Log Level">
              <select
                value={settings.logLevel}
                onChange={(e) => setSettings({ ...settings, logLevel: e.target.value })}
              >
                {["trace", "debug", "info", "warn", "error"].map((l) => (
                  <option key={l}>{l}</option>
                ))}
              </select>
            </Label>
            <p style={{ fontSize: 12, opacity: 0.6 }}>
              Changes take effect after restarting the server.
            </p>
          </div>
        ) : (
          <iframe
            src={dashboardUrl}
            title="Streamline Dashboard"
            style={{ width: "100%", height: "100%", border: "none" }}
          />
        )}
      </main>
    </div>
  );
}

function Label({ text, children }: { text: string; children: React.ReactNode }) {
  return (
    <label style={{ display: "block", marginBottom: 16 }}>
      <div style={{ marginBottom: 4, fontSize: 13, opacity: 0.8 }}>{text}</div>
      {children}
      <style>{`
        label input, label select {
          width: 100%;
          padding: 8px;
          border-radius: 4px;
          border: 1px solid #333;
          background: #1a1a2e;
          color: #eee;
          font-size: 14px;
          box-sizing: border-box;
        }
      `}</style>
    </label>
  );
}
