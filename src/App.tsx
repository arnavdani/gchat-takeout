import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface Group {
  id: number;
  google_id: string;
  name: string | null;
  group_type: string;
  message_count: number;
}

interface Message {
  id: number;
  user_name: string;
  user_email: string | null;
  text: string | null;
  created_at: string;
  topic_id: string | null;
}

function App() {
  const [groups, setGroups] = useState<Group[]>([]);
  const [selectedGroup, setSelectedGroup] = useState<Group | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadGroups();
  }, []);

  useEffect(() => {
    if (selectedGroup) {
      loadMessages(selectedGroup.id);
    }
  }, [selectedGroup]);

  useEffect(() => {
    scrollToBottom();
  }, [messages]);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  async function loadGroups() {
    try {
      const result = await invoke<Group[]>("get_groups");
      setGroups(result);
    } catch (err) {
      console.error("Failed to load groups:", err);
    }
  }

  async function loadMessages(groupId: number) {
    setLoading(true);
    try {
      const result = await invoke<Message[]>("get_messages", {
        groupId,
        limit: 100,
        offset: 0,
      });
      // Reverse because backend gives DESC, but we want to show bottom-up
      setMessages([...result].reverse());
    } catch (err) {
      console.error("Failed to load messages:", err);
    } finally {
      setLoading(false);
    }
  }

  async function handleImport() {
    setLoading(true);
    setStatus("Importing...");
    try {
      const result = await invoke<string>("import_takeout");
      setStatus(result);
      loadGroups();
    } catch (err) {
      setStatus(`Error: ${err}`);
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="app-container">
      <aside className="sidebar">
        <div className="sidebar-header">
          <h2>Chats</h2>
          <button onClick={handleImport} disabled={loading} className="import-btn">
            + Import
          </button>
        </div>
        <div className="group-list">
          {groups.map((group) => (
            <div
              key={group.id}
              className={`group-item ${selectedGroup?.id === group.id ? "active" : ""}`}
              onClick={() => setSelectedGroup(group)}
            >
              <div className="group-name">
                {group.name || (group.group_type === "DM" ? `DM ${group.google_id}` : `Space ${group.google_id}`)}
              </div>
              <div className="group-meta">
                {group.group_type} • {group.message_count} msgs
              </div>
            </div>
          ))}
        </div>
      </aside>

      <main className="main-view">
        {selectedGroup ? (
          <>
            <header className="chat-header">
              <h3>{selectedGroup.name || selectedGroup.google_id}</h3>
            </header>
            <div className="message-list">
              {messages.map((msg) => (
                <div key={msg.id} className="message-item">
                  <div className="message-user">
                    <strong>{msg.user_name}</strong>
                    <span className="message-date">{msg.created_at}</span>
                  </div>
                  <div className="message-text">{msg.text || <span className="attachment-label">📎 Attachment</span>}</div>
                </div>
              ))}
              <div ref={messagesEndRef} />
              {messages.length === 0 && !loading && <p className="empty-state">No messages in this chat.</p>}
            </div>
          </>
        ) : (
          <div className="welcome-state">
            <h1>Google Chat Browser</h1>
            <p>Select a chat from the sidebar or import new Takeout data.</p>
            {status && <p className="status-msg">{status}</p>}
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
