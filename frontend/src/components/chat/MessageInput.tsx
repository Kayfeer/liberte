import { useState, useRef } from "react";
import { Send, Paperclip } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useMessageStore } from "../../stores/messageStore";
import { sendFile, sendTyping } from "../../lib/tauri";
import { MAX_MESSAGE_LENGTH, TYPING_THROTTLE } from "../../lib/constants";
import { useFileStore } from "../../stores/fileStore";

export default function MessageInput({ channelId }: { channelId: string }) {
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [pending, setPending] = useState<Record<string, boolean>>({});
  const [errors, setErrors] = useState<Record<string, string | null>>({});
  const inFlight = useRef(new Set<string>());
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const activeChannel = useRef(channelId);
  activeChannel.current = channelId;
  const typingSentAt = useRef<Record<string, number>>({});
  const sendMessage = useMessageStore((state) => state.sendMessage);
  const content = drafts[channelId] || "";
  const sending = pending[channelId] || false;
  const error = errors[channelId];
  const setError = (id: string, value: string | null) => setErrors((current) => ({ ...current, [id]: value }));

  const handleSend = async () => {
    const trimmed = content.trim();
    if (!trimmed || inFlight.current.has(channelId)) return;
    inFlight.current.add(channelId);
    setPending((current) => ({ ...current, [channelId]: true }));
    setError(channelId, null);
    try {
      await sendMessage(channelId, trimmed);
      setDrafts((current) => current[channelId] === content ? { ...current, [channelId]: "" } : current);
      if (activeChannel.current === channelId) inputRef.current?.focus();
    } catch (reason) { setError(channelId, String(reason)); }
    finally {
      inFlight.current.delete(channelId);
      setPending((current) => ({ ...current, [channelId]: false }));
    }
  };

  const handleFileAttach = async () => {
    setError(channelId, null);
    try {
      const selected = await open({ multiple: false, title: "Joindre un fichier" });
      if (selected) {
        await sendFile(channelId, selected);
        await useFileStore.getState().load(channelId);
      }
    } catch (reason) { setError(channelId, String(reason)); }
  };

  return (
    <div className="px-4 pb-4">
      {error && <p role="alert" className="text-sm text-red-300 mb-2">{error}</p>}
      <div className="flex items-end gap-2 bg-liberte-surface rounded-lg border border-liberte-border p-2">
        <button onClick={() => void handleFileAttach()} aria-label="Joindre un fichier"
          className="p-2 text-liberte-muted hover:text-liberte-text transition-colors">
          <Paperclip className="w-5 h-5" />
        </button>
        <textarea ref={inputRef} value={content} aria-label="Message"
          onChange={(event) => {
            const value = event.target.value.slice(0, MAX_MESSAGE_LENGTH);
            setDrafts((current) => ({ ...current, [channelId]: value }));
            if (value.trim() && Date.now() - (typingSentAt.current[channelId] || 0) >= TYPING_THROTTLE) {
              typingSentAt.current[channelId] = Date.now();
              void sendTyping(channelId).catch(() => { /* Presence is best effort. */ });
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
              event.preventDefault();
              void handleSend();
            }
          }}
          placeholder="Envoyer un message chiffré..." rows={1}
          className="flex-1 bg-transparent resize-none outline-hidden text-sm text-liberte-text placeholder-liberte-muted py-2 max-h-32 overflow-y-auto" />
        <button onClick={() => void handleSend()} disabled={!content.trim() || sending} aria-label="Envoyer le message"
          className={`p-2 rounded-lg transition-colors ${content.trim() ? "text-liberte-accent hover:bg-liberte-panel" : "text-liberte-muted cursor-not-allowed"}`}>
          <Send className="w-5 h-5" />
        </button>
      </div>
      <div className="flex justify-between mt-1 px-1">
        <span className="text-xs text-liberte-muted">E2EE XChaCha20-Poly1305</span>
        <span className="text-xs text-liberte-muted">{content.length}/{MAX_MESSAGE_LENGTH}</span>
      </div>
    </div>
  );
}
