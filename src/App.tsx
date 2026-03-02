import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

interface Group {
  id: number;
  google_id: string;
  name: string | null;
  group_type: string;
  message_count: number;
  last_message_at: string | null;
}

interface Attachment {
  id: number;
  original_name: string;
  export_name: string;
  local_path: string;
}

interface Message {
  id: number;
  user_name: string;
  user_email: string | null;
  text: string | null;
  created_at: string;
  topic_id: string | null;
  attachments: Attachment[];
  is_me: bool;
}

function AttachmentView({ attachment }: { attachment: Attachment }) {
  const isImage = /\.(jpg|jpeg|png|gif|webp|bmp)$/i.test(attachment.export_name);
  const isVideo = /\.(mp4|mov|webm|ogg)$/i.test(attachment.export_name);
  const src = convertFileSrc(attachment.local_path);
  if (isImage) return <div className="attachment-image"><img src={src} alt={attachment.original_name} loading="lazy" /></div>;
  if (isVideo) return <div className="attachment-video"><video controls src={src} preload="metadata" /><div className="attachment-filename">{attachment.original_name}</div></div>;
  return <div className="attachment-file"><span className="file-icon">📄</span><span className="file-name">{attachment.original_name}</span></div>;
}

function App() {
  const [groups, setGroups] = useState<Group[]>([]);
  const [selectedGroup, setSelectedGroup] = useState<Group | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [members, setMembers] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState("");
  
  const [groupQuery, setGroupQuery] = useState("");
  const [messageQuery, setMessageQuery] = useState("");
  const [isMsgSearchOpen, setIsMsgSearchOpen] = useState(false);
  const [focusArea, setFocusArea] = useState<"sidebar" | "main">("sidebar");
  const [showSettings, setShowSettings] = useState(false);
  
  const [theme, setTheme] = useState<"light" | "dark">(() => {
    return (localStorage.getItem("gchat-theme") as "light" | "dark") || "dark";
  });
  
  const [hasMore, setHasMore] = useState(true);
  const PAGE_SIZE = 200;

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messageListRef = useRef<HTMLDivElement>(null);
  const groupSearchRef = useRef<HTMLInputElement>(null);
  const messageSearchRef = useRef<HTMLInputElement>(null);
  const sidebarRef = useRef<HTMLElement>(null);
  const groupRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  const [sidebarWidth, setSidebarWidth] = useState(300);
  const isResizing = useRef(false);

  useEffect(() => {
    const unlisten = listen("show-settings", () => {
      setShowSettings(prev => !prev);
      setSelectedGroup(null);
    });
    return () => { unlisten.then(f => f()); };
  }, []);

  useEffect(() => {
    invoke<string | null>("get_config", { key: "theme" }).then(val => {
      if (val === "light" || val === "dark") {
        setTheme(val);
        localStorage.setItem("gchat-theme", val);
      }
    });
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("gchat-theme", theme);
    invoke("set_config", { key: "theme", value: theme });
  }, [theme]);

  useEffect(() => { loadGroups(); }, [groupQuery]);
  
  useEffect(() => { 
    if (selectedGroup) {
      setMessages([]);
      setHasMore(true);
      loadMessages(selectedGroup.id, 0, true);
      loadMembers(selectedGroup.id);
      const el = groupRefs.current.get(selectedGroup.id);
      if (el) el.scrollIntoView({ block: "nearest" });
    } 
  }, [selectedGroup]);

  useEffect(() => {
    if (selectedGroup) {
      setHasMore(!messageQuery);
      loadMessages(selectedGroup.id, 0, true);
    }
  }, [messageQuery]);

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    if (target.scrollTop === 0 && !loading && hasMore && selectedGroup && !messageQuery) {
      const currentScrollHeight = target.scrollHeight;
      loadMessages(selectedGroup.id, messages.length, false).then(() => {
        setTimeout(() => { target.scrollTop = target.scrollHeight - currentScrollHeight; }, 0);
      });
    }
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        if (focusArea === "main" && selectedGroup) {
          if (isMsgSearchOpen && document.activeElement === messageSearchRef.current) {
            setIsMsgSearchOpen(false); setMessageQuery("");
          } else {
            setIsMsgSearchOpen(true); setTimeout(() => messageSearchRef.current?.focus(), 50);
          }
        } else {
          setFocusArea("sidebar"); groupSearchRef.current?.focus();
        }
        return;
      }
      
      if (focusArea === "sidebar") {
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          const currentIndex = groups.findIndex(g => g.id === selectedGroup?.id);
          let nextIndex = currentIndex;
          if (e.key === "ArrowDown") nextIndex = currentIndex === -1 ? 0 : Math.min(groups.length - 1, currentIndex + 1);
          else if (e.key === "ArrowUp") nextIndex = currentIndex === -1 ? 0 : Math.max(0, currentIndex - 1);
          if (nextIndex !== -1 && groups[nextIndex]) handleSelectGroup(groups[nextIndex], false);
        }
        if (e.key === "ArrowRight" && selectedGroup) { e.preventDefault(); setFocusArea("main"); }
      } else if (focusArea === "main") {
        if (e.key === "ArrowLeft") {
          e.preventDefault(); setFocusArea("sidebar"); setIsMsgSearchOpen(false); setMessageQuery("");
        }
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          if (document.activeElement?.tagName !== "INPUT") {
            e.preventDefault();
            const amount = e.key === "ArrowDown" ? 450 : -450;
            messageListRef.current?.scrollBy({ top: amount, behavior: "auto" });
          }
        }
      }

      if (e.key === "Escape") {
        if (isMsgSearchOpen) { setIsMsgSearchOpen(false); setMessageQuery(""); }
        if (groupQuery) { setGroupQuery(""); }
        if (showSettings) setShowSettings(false);
      }

      if (e.key === "Enter" && document.activeElement === groupSearchRef.current && groups.length > 0) {
        handleSelectGroup(groups[0], true); groupSearchRef.current?.blur();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [selectedGroup, isMsgSearchOpen, groups, groupQuery, focusArea, hasMore, messages.length, showSettings]);

  const handleSelectGroup = (group: Group, shouldFocusMain = true) => {
    setSelectedGroup(group);
    setShowSettings(false);
    if (shouldFocusMain) setFocusArea("main");
  };

  const startResizing = useCallback(() => {
    isResizing.current = true;
    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", stopResizing);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, []);

  const stopResizing = useCallback(() => {
    isResizing.current = false;
    document.removeEventListener("mousemove", handleMouseMove);
    document.removeEventListener("mouseup", stopResizing);
    document.body.style.cursor = "default";
    document.body.style.userSelect = "auto";
  }, []);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isResizing.current) return;
    const newWidth = e.clientX;
    if (newWidth > 150 && newWidth < 600) setSidebarWidth(newWidth);
  }, []);

  const scrollToBottom = () => { messagesEndRef.current?.scrollIntoView({ behavior: "auto" }); };

  async function loadGroups() {
    try {
      const result = await invoke<Group[]>("get_groups", { query: groupQuery || null });
      setGroups(result);
    } catch (err) { console.error(err); }
  }

  async function loadMembers(groupId: number) {
    try {
      const result = await invoke<string[]>("get_group_members", { groupId });
      setMembers(result);
    } catch (err) { console.error(err); }
  }

  async function loadMessages(groupId: number, offset: number, isInitial: boolean) {
    setLoading(true);
    try {
      const result = await invoke<Message[]>("get_messages", {
        groupId, limit: PAGE_SIZE, offset, query: messageQuery || null
      });
      const reversed = [...result].reverse();
      if (isInitial) {
        setMessages(reversed);
        setTimeout(scrollToBottom, 50);
      } else {
        setMessages(prev => [...reversed, ...prev]);
      }
      if (result.length < PAGE_SIZE) setHasMore(false);
    } catch (err) { console.error(err); } finally { setLoading(false); }
  }

  async function handleImport() {
    setLoading(true); setStatus("Importing...");
    try {
      const result = await invoke<string>("import_takeout");
      setStatus(result); loadGroups();
    } catch (err) { setStatus(`Error: ${err}`); } finally { setLoading(false); }
  }

  return (
    <div className={`app-container focus-${focusArea}`}>
      <aside ref={sidebarRef} className="sidebar" style={{ width: sidebarWidth }} onClick={(e) => { e.stopPropagation(); setFocusArea("sidebar"); }}>
        <div className="sidebar-header-main">
          <div className="sidebar-top">
            <h2 onClick={() => { setShowSettings(false); setSelectedGroup(null); }} style={{cursor: "pointer"}}>Chats</h2>
            <div className="sidebar-actions">
              <button onClick={handleImport} disabled={loading} className="import-btn">+ Import</button>
            </div>
          </div>
          <div className="search-box">
            <input ref={groupSearchRef} type="text" placeholder="Search chats..." value={groupQuery} onChange={(e) => setGroupQuery(e.target.value)} onFocus={() => setFocusArea("sidebar")} />
          </div>
        </div>
        <div className="group-list">
          {groups.map((group) => (
            <div key={group.id} ref={el => { if (el) groupRefs.current.set(group.id, el); else groupRefs.current.delete(group.id); }} className={`group-item ${selectedGroup?.id === group.id ? "active" : ""}`} onClick={() => handleSelectGroup(group, true)}>
              <div className="group-name">{group.name || (group.group_type === "DM" ? `DM ${group.google_id}` : `Space ${group.google_id}`)}</div>
              <div className="group-meta">{group.group_type} • {group.message_count} msgs</div>
            </div>
          ))}
        </div>
      </aside>
      <div className="resizer" onMouseDown={startResizing} />
      
      <main className="main-view" onClick={() => setFocusArea("main")}>
        {showSettings ? (
          <div className="settings-view">
            <header className="chat-header"><h3>Settings</h3></header>
            <div className="settings-content">
              <div className="setting-item">
                <label>Theme</label>
                <div className="theme-toggle">
                  <button className={theme === 'light' ? 'active' : ''} onClick={() => setTheme('light')}>Light</button>
                  <button className={theme === 'dark' ? 'active' : ''} onClick={() => setTheme('dark')}>Dark</button>
                </div>
              </div>
              <div className="settings-info">
                <p>Database: <code>~/Library/Application Support/com.arnavdani.gchat-takeout/</code></p>
              </div>
            </div>
          </div>
        ) : selectedGroup ? (
          <div className="chat-container">
            <header className="chat-header">
              <div className="header-left">
                <h3>{selectedGroup.name || selectedGroup.google_id}</h3>
                <div className="members-badge">{members.length} members<div className="members-tooltip"><h4>Members</h4><ul>{members.map((m, i) => <li key={i}>{m}</li>)}</ul></div></div>
              </div>
              <div className="header-right">
                <div className={`message-search-wrapper ${isMsgSearchOpen ? 'open' : ''}`}>
                  {!isMsgSearchOpen && <button className="search-toggle-btn" onClick={() => { setIsMsgSearchOpen(true); setTimeout(() => messageSearchRef.current?.focus(), 50); }}>🔍</button>}
                  <div className="message-search-input-container">
                    <input ref={messageSearchRef} type="text" placeholder="Search..." value={messageQuery} onChange={(e) => setMessageQuery(e.target.value)} onFocus={() => setFocusArea("main")} onBlur={() => { if (!messageQuery) setIsMsgSearchOpen(false); }} />
                    {isMsgSearchOpen && <button className="close-search-btn" onClick={() => { setIsMsgSearchOpen(false); setMessageQuery(""); }}>✕</button>}
                  </div>
                </div>
              </div>
            </header>
            <div className="message-list" ref={messageListRef} onScroll={handleScroll}>
              {loading && hasMore && <div className="loading-more">Loading more history...</div>}
              {messages.map((msg) => (
                <div key={msg.id} className={`message-item ${msg.is_me ? 'me' : 'them'}`}>
                  {!msg.is_me && <div className="message-user"><strong>{msg.user_name}</strong></div>}
                  <div className="message-bubble">
                    {msg.text && <div className="message-text">{msg.text}</div>}
                    {msg.attachments?.length > 0 && <div className="message-attachments">{msg.attachments.map((att) => <AttachmentView key={att.id} attachment={att} />)}</div>}
                    <div className="message-date">{msg.created_at}</div>
                  </div>
                </div>
              ))}
              <div ref={messagesEndRef} />
              {messages.length === 0 && !loading && <p className="empty-state">{messageQuery ? "No matches found." : "No messages found."}</p>}
            </div>
          </div>
        ) : (
          <div className="welcome-state"><h1>Google Chat Browser</h1><p>Select a chat from the sidebar or import new Takeout data.</p>{status && <p className="status-msg">{status}</p>}</div>
        )}
      </main>
    </div>
  );
}

export default App;
