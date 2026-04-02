import { useState, useRef, useEffect, useCallback } from "react";

/* ──────────────── DATA ──────────────── */
const SESSIONS = [
  { id: 1, name: "plane", branch: "fix/pdf-export_", shell: "pwsh", dot: "#f59e0b" },
  { id: 2, name: "opensessions", branch: "main", shell: "pwsh", dot: "#22c55e", active: true },
  { id: 3, name: "quiver", branch: "main", shell: "bastd", dot: "#22c55e" },
];

const TABS = [
  { id: 1, name: "opensessions", color: "#6366f1" },
  { id: 2, name: "opensessions", color: "#10b981", badge: 3 },
  { id: 3, name: "work", color: "#f97316" },
  { id: 4, name: "opensessions", color: "#8b5cf6", badge: 12 },
  { id: 5, name: "opensessions", color: "#6366f1" },
];

const BOTTOM_PROCESSES = [
  { icon: "!", name: "amp", status: "running", color: "#22c55e", desc: "Verify and clean README documentation", dismiss: true },
  { icon: "⚠", name: "amp", status: "stopped", color: "#ef4444", desc: "Verify README against codebase", dismiss: true },
  { icon: "●", name: "claude-code", status: "waiting", color: "#6366f1", dismiss: true },
];

const BOTTOM_ACTIONS = ["~ cycle", "⊘ go", "d remove", "u restore", "x kill", "t theme"];

const RIGHT_PANEL_CONTENT = {
  title: "one more",
  poem: {
    title: "The Gardener of Broken Things",
    stanzas: [
      "I keep a workshop in my chest\nwhere bent and rusted hours collect,\nwhere Mondays that did not go well\nsit next to plans I didn't protect.",
      "There's a drawer of almost-good-enough,\na shelf of words I should have said,\na box of mornings lost to doubt,\na jar of thoughts I overfed.",
      "But I have learned — not all at once,\nnot in a flash of brilliant light,\nbut slowly, like a vine that climbs\na wall it cannot see at night —",
      "that broken things still hold their shape.\nA cracked cup knows what it can pour.\nA fraying rope still understands\nthe weight it used to carry before.",
      "So I sit down with careful hands\nand turn each piece against the light,\nnot asking it to be brand new,\nbut asking it to feel less tight.",
      "I oil the hinge of an old regret.\nI sand the edge of a clumsy year.\nI don't rebuild — I just make room\nfor what was always living here.",
      "Some people throw their damage out,\nreplace it all with polished chrome.\nBut I prefer the dents and scuffs —\nthey're how I recognize my home.",
      "The workshop hums. The lantern sways.\nI mend what I can mend, and then\nI set the broken clock to now\nand let the whole thing start again.",
    ],
    footer: "Hope you enjoyed that one too.",
  },
};

/* Rich content for the main terminal */
const RICH_CONTENT = [
  {
    type: "bullet",
    text: "Tightened the support story so the README stays clearly tmux-first while still acknowledging the experimental zellij code in-repo.",
  },
  { type: "heading", text: "Verification Notes" },
  {
    type: "bullet",
    children: [
      { text: "The rewritten ", link: null },
      { text: "README.md", link: true },
      { text: " now reflects the actual sidebar surface:", link: null },
    ],
    sub: [
      "session list shows branch and agent status",
      "detail panel shows working directory, ports, agent rows, and thread names",
      "users can hide/restore/kill/reorder sessions and switch themes",
      "detected localhost ports open from the UI",
    ],
  },
  {
    type: "bullet",
    children: [
      { text: "I also verified the smoke-test path directly: " },
      { text: "bun test", code: true },
      { text: " from the repo root passes right now with 167 passing tests." },
    ],
  },
  { type: "heading", text: "One Residual Note" },
  {
    type: "paragraph",
    text: "I only changed the README. While auditing, I noticed some secondary docs still have minor drift from the current UI behavior, especially the feature/keybinding docs.",
  },
  {
    type: "numbered",
    children: [
      { text: "If you want, I can do the same line-by-line cleanup pass on " },
      { text: "docs/reference/features-and-keybindings.md", link: true },
      { text: " and the rest of " },
      { text: "docs/", code: true },
      { text: " next." },
    ],
  },
  {
    type: "user-message",
    text: "also make it a bit more fun! (interrupted)",
    highlight: "#c4b28a",
  },
  {
    type: "user-message",
    text: "and compact!",
    highlight: "#c4b28a",
  },
  {
    type: "thoughts",
    count: 2,
    items: ["Editing README for clarity", "Streamlining README structure"],
  },
  {
    type: "paragraph",
    text: 'I\'m tightening the README again, this time for tone and density: shorter sections, less repetition, and a little more personality without getting cute enough to obscure what the project does.',
  },
  {
    type: "command",
    text: "$ rtk proxy nl -ba README.md | sed -n '1,220p'",
  },
  {
    type: "thoughts",
    count: 2,
    items: ["Condensing the file", "Simplifying the README"],
  },
  {
    type: "paragraph",
    text: 'I\'m collapsing the README structure now: fewer headings, less "verification report" tone, and more "here\'s why this is useful" energy. The goal is that the first screen reads fast and still feels grounded in the actual code.',
  },
  {
    type: "editing",
    text: "Editing files",
  },
];

/* ──────────────── COMPONENTS ──────────────── */

function StatusDot({ color, size = 7 }) {
  return (
    <span
      style={{
        width: size,
        height: size,
        borderRadius: "50%",
        backgroundColor: color,
        display: "inline-block",
        flexShrink: 0,
      }}
    />
  );
}

function Badge({ count, color }) {
  if (!count) return null;
  return (
    <span
      style={{
        backgroundColor: color,
        color: "#fff",
        fontSize: 9,
        fontWeight: 700,
        borderRadius: 7,
        padding: "1px 5px",
        minWidth: 16,
        height: 16,
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        lineHeight: 1,
      }}
    >
      {count}
    </span>
  );
}

function ThoughtsBlock({ count, items, defaultOpen = false }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div style={{ margin: "8px 0 6px" }}>
      <div
        onClick={() => setOpen(!open)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          cursor: "pointer",
          color: "#6e7681",
          fontSize: 12,
          userSelect: "none",
        }}
      >
        <span style={{ color: "#22c55e", fontSize: 13 }}>✓</span>
        <span>{count} thoughts {open ? "▾" : "▸"}</span>
      </div>
      {open && (
        <div style={{ paddingLeft: 20, marginTop: 4 }}>
          {items.map((item, i) => (
            <div
              key={i}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 6,
                color: "#484f58",
                fontSize: 12,
                padding: "1px 0",
              }}
            >
              <span style={{ color: "#3b4048" }}>·</span>
              <span>{item}</span>
              <span style={{ color: "#3b4048", marginLeft: 2 }}>▸</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function InlineContent({ children }) {
  if (!children) return null;
  return (
    <span>
      {children.map((c, i) => {
        if (c.link) {
          return (
            <span key={i} style={{ color: "#58a6ff", textDecoration: "underline", cursor: "pointer" }}>
              {c.text}
            </span>
          );
        }
        if (c.code) {
          return (
            <span
              key={i}
              style={{
                backgroundColor: "#1c2030",
                color: "#e6a855",
                padding: "1px 5px",
                borderRadius: 3,
                fontSize: 12,
                fontFamily: "inherit",
              }}
            >
              {c.text}
            </span>
          );
        }
        return <span key={i}>{c.text}</span>;
      })}
    </span>
  );
}

function RichBlock({ block }) {
  if (block.type === "heading") {
    return (
      <h3
        style={{
          color: "#e6edf3",
          fontSize: 14,
          fontWeight: 700,
          margin: "18px 0 8px",
          fontFamily: "inherit",
          letterSpacing: 0.2,
        }}
      >
        {block.text}
      </h3>
    );
  }

  if (block.type === "bullet") {
    return (
      <div style={{ display: "flex", gap: 8, margin: "4px 0", lineHeight: 1.55, fontSize: 13 }}>
        <span style={{ color: "#6e7681", flexShrink: 0 }}>•</span>
        <div>
          {block.children ? <InlineContent children={block.children} /> : <span>{block.text}</span>}
          {block.sub && (
            <div style={{ paddingLeft: 12, marginTop: 2 }}>
              {block.sub.map((s, i) => (
                <div key={i} style={{ display: "flex", gap: 8, color: "#8b949e", fontSize: 12, padding: "1px 0" }}>
                  <span style={{ color: "#484f58" }}>•</span>
                  <span>{s}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  }

  if (block.type === "numbered") {
    return (
      <div style={{ display: "flex", gap: 8, margin: "4px 0", lineHeight: 1.55, fontSize: 13 }}>
        <span style={{ color: "#6e7681", flexShrink: 0 }}>1.</span>
        <span>
          <InlineContent children={block.children} />
        </span>
      </div>
    );
  }

  if (block.type === "paragraph") {
    return (
      <p style={{ margin: "8px 0", lineHeight: 1.6, fontSize: 13, color: "#c9d1d9" }}>{block.text}</p>
    );
  }

  if (block.type === "user-message") {
    return (
      <div
        style={{
          borderLeft: "3px solid #6366f1",
          backgroundColor: "#14171f",
          padding: "6px 12px",
          margin: "6px 0",
          borderRadius: "0 4px 4px 0",
          color: block.highlight || "#c9d1d9",
          fontSize: 13,
          fontWeight: 500,
        }}
      >
        {block.text}
      </div>
    );
  }

  if (block.type === "thoughts") {
    return <ThoughtsBlock count={block.count} items={block.items} />;
  }

  if (block.type === "command") {
    return (
      <div
        style={{
          backgroundColor: "#0a0c10",
          border: "1px solid #1e2128",
          borderRadius: 5,
          padding: "6px 10px",
          margin: "8px 0",
          fontSize: 12,
          color: "#8b949e",
          display: "flex",
          alignItems: "center",
          gap: 6,
        }}
      >
        <span style={{ color: "#6e7681", fontFamily: "inherit" }}>{block.text}</span>
        <span style={{ color: "#3b4048", marginLeft: "auto" }}>▸</span>
      </div>
    );
  }

  if (block.type === "editing") {
    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          color: "#6e7681",
          fontSize: 12,
          margin: "6px 0",
        }}
      >
        <span style={{ color: "#3b4048" }}>::</span>
        <span>{block.text}</span>
        <span style={{ color: "#3b4048" }}>▸</span>
      </div>
    );
  }

  return null;
}

function ProgressBar() {
  const [width, setWidth] = useState(5);
  useEffect(() => {
    const timer = setInterval(() => {
      setWidth((w) => (w >= 90 ? 5 : w + Math.random() * 2));
    }, 300);
    return () => clearInterval(timer);
  }, []);
  return (
    <div style={{ height: 2, backgroundColor: "#1e2128", width: "100%", overflow: "hidden" }}>
      <div
        style={{
          height: "100%",
          width: `${width}%`,
          background: "linear-gradient(90deg, #6366f1, #8b5cf6, #6366f1)",
          transition: "width 0.3s ease",
          borderRadius: 1,
        }}
      />
    </div>
  );
}

/* ──────────────── SCROLLBAR STYLES ──────────────── */
const scrollCSS = `
  * { box-sizing: border-box; }
  ::-webkit-scrollbar { width: 6px; height: 6px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background: #2d333b; border-radius: 3px; }
  ::-webkit-scrollbar-thumb:hover { background: #3b4048; }
  @keyframes blink { 0%,100%{opacity:1} 50%{opacity:0} }
  @keyframes pulse { 0%,100%{opacity:0.6} 50%{opacity:1} }
  @keyframes spin { 0%{transform:rotate(0deg)} 100%{transform:rotate(360deg)} }
`;

/* ──────────────── MAIN COMPONENT ──────────────── */

export default function GodlyTerminal() {
  const [activeTab, setActiveTab] = useState(2);
  const [activeSession, setActiveSession] = useState(2);
  const [sidebarWidth, setSidebarWidth] = useState(200);
  const [rightPanelWidth, setRightPanelWidth] = useState(380);
  const [showRight, setShowRight] = useState(true);
  const [streaming, setStreaming] = useState(true);
  const mainRef = useRef(null);

  /* Sidebar resize */
  const startLeftResize = useCallback(() => {
    const onMove = (e) => setSidebarWidth(Math.max(150, Math.min(320, e.clientX)));
    const onUp = () => { document.removeEventListener("mousemove", onMove); document.removeEventListener("mouseup", onUp); };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }, []);

  /* Right panel resize */
  const startRightResize = useCallback(() => {
    const onMove = (e) => setRightPanelWidth(Math.max(250, Math.min(550, window.innerWidth - e.clientX)));
    const onUp = () => { document.removeEventListener("mousemove", onMove); document.removeEventListener("mouseup", onUp); };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }, []);

  useEffect(() => {
    if (mainRef.current) mainRef.current.scrollTop = mainRef.current.scrollHeight;
  }, []);

  const font = "'JetBrains Mono','Cascadia Code','Fira Code','Consolas',monospace";

  return (
    <div style={{ width: "100%", height: "100vh", backgroundColor: "#0b0d12", color: "#c9d1d9", fontFamily: font, fontSize: 13, display: "flex", flexDirection: "column", overflow: "hidden" }}>
      <style>{scrollCSS}</style>

      {/* ──── TAB BAR ──── */}
      <div style={{ height: 36, backgroundColor: "#0f1117", display: "flex", alignItems: "stretch", borderBottom: "1px solid #1a1d25", flexShrink: 0, paddingLeft: 2 }}>
        {TABS.map((tab, idx) => {
          const active = activeTab === tab.id && idx === 1;
          return (
            <div
              key={idx}
              onClick={() => setActiveTab(tab.id)}
              style={{
                display: "flex", alignItems: "center", gap: 6, padding: "0 14px",
                cursor: "pointer",
                borderBottom: active ? `2px solid ${tab.color}` : "2px solid transparent",
                backgroundColor: active ? "#161920" : "transparent",
                color: active ? "#e6edf3" : "#555d6b",
                fontSize: 12, whiteSpace: "nowrap", transition: "all 0.15s",
              }}
            >
              <span style={{
                width: 18, height: 18, borderRadius: "50%",
                background: `${tab.color}22`,
                color: tab.color,
                fontSize: 10, fontWeight: 700,
                display: "flex", alignItems: "center", justifyContent: "center",
              }}>
                {idx + 1}
              </span>
              <span style={{ fontWeight: active ? 600 : 400 }}>{tab.name}</span>
              {tab.badge && <Badge count={tab.badge} color={tab.color} />}
            </div>
          );
        })}
        <div style={{ flex: 1 }} />
        {/* Right-side tab icons */}
        <div style={{ display: "flex", alignItems: "center", gap: 10, paddingRight: 14, fontSize: 11, color: "#555d6b" }}>
          <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <span style={{ fontSize: 13 }}>🟠</span>
            <span style={{ fontWeight: 600 }}>bun</span>
          </span>
          <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <span style={{ width: 8, height: 8, borderRadius: "50%", backgroundColor: "#22c55e", display: "inline-block" }} />
            <span style={{ fontWeight: 600 }}>opensessions</span>
          </span>
        </div>
      </div>

      {/* ──── MAIN AREA ──── */}
      <div style={{ display: "flex", flex: 1, overflow: "hidden" }}>

        {/* ──── LEFT SIDEBAR ──── */}
        <div style={{ width: sidebarWidth, backgroundColor: "#0b0d12", borderRight: "1px solid #1a1d25", display: "flex", flexDirection: "column", flexShrink: 0, overflow: "hidden" }}>

          {/* Session count */}
          <div style={{ padding: "12px 14px 4px", display: "flex", alignItems: "center", gap: 6, fontSize: 12, color: "#6e7681" }}>
            Sessions {SESSIONS.length}
            <span style={{ color: "#484f58", fontSize: 10 }}>⚡ 1</span>
          </div>

          {/* Session list */}
          <div style={{ flex: 1, overflowY: "auto", padding: "4px 6px" }}>
            {SESSIONS.map((s) => {
              const isActive = activeSession === s.id;
              return (
                <div
                  key={s.id}
                  onClick={() => setActiveSession(s.id)}
                  style={{
                    padding: "7px 8px",
                    borderRadius: 6,
                    cursor: "pointer",
                    backgroundColor: isActive ? "#171b24" : "transparent",
                    borderLeft: isActive ? "3px solid #6366f1" : "3px solid transparent",
                    marginBottom: 2,
                    transition: "all 0.12s",
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <span style={{ color: "#555d6b", fontSize: 12, fontWeight: 500, minWidth: 10 }}>{s.id}</span>
                    <span style={{ fontWeight: 600, color: isActive ? "#e6edf3" : "#9198a1", fontSize: 13 }}>{s.name}</span>
                    {isActive && <span style={{ color: "#484f58", marginLeft: "auto", fontSize: 11 }}>::</span>}
                  </div>
                  <div style={{ paddingLeft: 20, fontSize: 11, color: "#484f58", marginTop: 2 }}>
                    {s.branch}
                  </div>
                </div>
              );
            })}
          </div>

          {/* Bottom processes area */}
          <div style={{ borderTop: "1px solid #1a1d25", maxHeight: 220, overflowY: "auto" }}>
            <div style={{ padding: "8px 10px 4px", fontSize: 10, color: "#484f58", letterSpacing: 0.5 }}>
              …ments/work/opensessions
            </div>
            {BOTTOM_PROCESSES.map((p, i) => (
              <div
                key={i}
                style={{
                  padding: "5px 10px",
                  display: "flex",
                  flexDirection: "column",
                  gap: 2,
                  borderBottom: "1px solid #13161d",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12 }}>
                  <span style={{ color: p.color, fontSize: 11 }}>{p.icon === "!" ? "ⓘ" : p.icon === "⚠" ? "⚠" : "●"}</span>
                  <span style={{ color: "#9198a1", fontWeight: 600 }}>{p.name}</span>
                  <span style={{
                    fontSize: 10, fontWeight: 600, color: p.color,
                    backgroundColor: p.color + "18",
                    padding: "1px 6px", borderRadius: 3,
                  }}>
                    {p.status}
                  </span>
                  {p.dismiss && <span style={{ color: "#3b4048", marginLeft: "auto", cursor: "pointer", fontSize: 13 }}>×</span>}
                </div>
                {p.desc && (
                  <div style={{ fontSize: 11, color: "#484f58", paddingLeft: 20, lineHeight: 1.3 }}>{p.desc}</div>
                )}
              </div>
            ))}
          </div>

          {/* Action shortcuts */}
          <div style={{
            borderTop: "1px solid #1a1d25",
            padding: "6px 10px",
            display: "flex",
            flexWrap: "wrap",
            gap: "4px 10px",
            fontSize: 10,
            color: "#3b4048",
          }}>
            {BOTTOM_ACTIONS.map((a, i) => (
              <span key={i} style={{ cursor: "pointer", whiteSpace: "nowrap" }}>{a}</span>
            ))}
          </div>
        </div>

        {/* Left resize handle */}
        <div
          onMouseDown={startLeftResize}
          style={{ width: 3, cursor: "col-resize", flexShrink: 0, backgroundColor: "transparent", transition: "background 0.15s" }}
          onMouseEnter={(e) => e.target.style.backgroundColor = "#2d333b"}
          onMouseLeave={(e) => e.target.style.backgroundColor = "transparent"}
        />

        {/* ──── CENTER: MAIN TERMINAL ──── */}
        <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden", minWidth: 0 }}>

          {/* Rich terminal content */}
          <div
            ref={mainRef}
            style={{
              flex: 1,
              overflowY: "auto",
              padding: "12px 20px 20px",
              backgroundColor: "#0e1017",
              lineHeight: 1.5,
            }}
          >
            {RICH_CONTENT.map((block, i) => (
              <RichBlock key={i} block={block} />
            ))}

            {/* Streaming cursor */}
            {streaming && (
              <span
                style={{
                  display: "inline-block",
                  width: 8,
                  height: 16,
                  backgroundColor: "#6366f1",
                  animation: "blink 1s infinite",
                  borderRadius: 1,
                  marginTop: 8,
                  verticalAlign: "middle",
                }}
              />
            )}
          </div>

          {/* Progress bar */}
          {streaming && <ProgressBar />}

          {/* Bottom status bar */}
          <div style={{
            height: 26, backgroundColor: "#0c0e14",
            borderTop: "1px solid #1a1d25",
            display: "flex", alignItems: "center",
            padding: "0 14px", fontSize: 11, color: "#484f58",
            flexShrink: 0, gap: 6,
          }}>
            {streaming ? (
              <>
                <span style={{ display: "flex", alignItems: "center", gap: 5, color: "#8b949e" }}>
                  <span style={{ animation: "pulse 1.5s infinite", fontSize: 9 }}>~</span>
                  Streaming response...
                </span>
                <span style={{ color: "#6e7681", marginLeft: 12 }}>Esc to cancel</span>
              </>
            ) : (
              <span>Ready</span>
            )}
            <div style={{ flex: 1 }} />

            {/* Path + branch */}
            <span style={{ color: "#3b4048" }}>~/Documents/work/opensessions</span>
            <span style={{ color: "#2d333b" }}>|</span>
            <span style={{ color: "#f59e0b" }}>(main)</span>
            <span style={{ color: "#2d333b" }}>|</span>

            {/* Git diff stats */}
            <span>1 file changed</span>
            <span style={{ color: "#22c55e" }}>+21</span>
            <span style={{ color: "#ef4444" }}>~4</span>
            <span style={{ color: "#ef4444" }}>-70</span>
          </div>
        </div>

        {/* Right resize handle */}
        {showRight && (
          <div
            onMouseDown={startRightResize}
            style={{ width: 3, cursor: "col-resize", flexShrink: 0, backgroundColor: "transparent", transition: "background 0.15s" }}
            onMouseEnter={(e) => e.target.style.backgroundColor = "#2d333b"}
            onMouseLeave={(e) => e.target.style.backgroundColor = "transparent"}
          />
        )}

        {/* ──── RIGHT PANEL ──── */}
        {showRight && (
          <div style={{ width: rightPanelWidth, backgroundColor: "#0b0d12", borderLeft: "1px solid #1a1d25", display: "flex", flexDirection: "column", flexShrink: 0, overflow: "hidden" }}>

            {/* Right panel header */}
            <div style={{ padding: "10px 14px", borderBottom: "1px solid #1a1d25", display: "flex", alignItems: "center", gap: 8, fontSize: 12 }}>
              <span style={{ color: "#484f58" }}>{RIGHT_PANEL_CONTENT.title}</span>
              <div style={{ flex: 1 }} />
              <span
                onClick={() => setShowRight(false)}
                style={{ color: "#3b4048", cursor: "pointer", fontSize: 14 }}
              >
                ×
              </span>
            </div>

            {/* Poem content */}
            <div style={{ flex: 1, overflowY: "auto", padding: "16px 20px" }}>
              {/* Title */}
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 16 }}>
                <StatusDot color="#e6edf3" size={8} />
                <span style={{ fontSize: 15, fontWeight: 700, color: "#e6edf3", letterSpacing: 0.3 }}>
                  {RIGHT_PANEL_CONTENT.poem.title}
                </span>
              </div>

              {/* Stanzas */}
              {RIGHT_PANEL_CONTENT.poem.stanzas.map((stanza, i) => (
                <div
                  key={i}
                  style={{
                    marginBottom: 18,
                    lineHeight: 1.7,
                    fontSize: 13,
                    color: "#9198a1",
                    whiteSpace: "pre-wrap",
                    fontFamily: "'Georgia','Times New Roman',serif",
                    fontStyle: "italic",
                    letterSpacing: 0.2,
                  }}
                >
                  {stanza}
                </div>
              ))}

              {/* Divider + footer */}
              <div style={{ borderTop: "1px solid #1a1d25", paddingTop: 12, marginTop: 8, color: "#6e7681", fontSize: 12 }}>
                {RIGHT_PANEL_CONTENT.poem.footer}
              </div>
            </div>

            {/* Right panel status bar */}
            <div style={{
              height: 26, backgroundColor: "#0c0e14",
              borderTop: "1px solid #1a1d25",
              display: "flex", alignItems: "center", justifyContent: "flex-end",
              padding: "0 14px", fontSize: 11, color: "#3b4048",
              flexShrink: 0,
            }}>
              <span>{"}"}</span>
              <div style={{ flex: 1 }} />
              <span>? for shortcuts</span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
