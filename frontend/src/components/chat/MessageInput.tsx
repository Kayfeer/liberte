import { useState, useRef } from "react";
import { Send, Paperclip } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useMessageStore } from "../../stores/messageStore";
import { sendFile, sendTyping } from "../../lib/tauri";
import { MAX_MESSAGE_LENGTH, TYPING_THROTTLE } from "../../lib/constants";
import { useFileStore } from "../../stores/fileStore";

interface Props {
  channelId: string;
}

export default function MessageInput({ channelId }: Props) {
  const [content, setContent] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const typingSentAt = useRef(0);
  const { sendMessage } = useMessageStore();

  const handleSend = async () => {
    const trimmed = content.trim();
    if (!trimmed || sending) return;

    setSending(true);
    setError(null);
    try {
      await sendMessage(channelId, trimmed);
      setContent("");
      inputRef.current?.focus();
    } catch (e) {
      setError(String(e));
    } finally {
      setSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleFileAttach = async () => {
    setError(null);
    try {
      const selected = await open({
        multiple: false,
        title: "Joindre un fichier",
      });
      if (selected) {
        await sendFile(channelId, selected);
        await useFileStore.getState().load(channelId);
      }
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="px-4 pb-4">
      {error && <p role="alert" className="text-sm text-red-300 mb-2">{error}</p>}
      <div className="flex items-end gap-2 bg-liberte-surface rounded-lg border border-liberte-border p-2">
        <button
          onClick={handleFileAttach}
          className="p-2 text-liberte-muted hover:text-liberte-text transition-colors"
        >
          <Paperclip className="w-5 h-5" />
        </button>

        <textarea
          ref={inputRef}
          value={content}
          onChange={(e) => {
            const value = e.target.value.slice(0, MAX_MESSAGE_LENGTH);
            setContent(value);
            if (value.trim() && Date.now() - typingSentAt.current >= TYPING_THROTTLE) {
              typingSentAt.current = Date.now();
              void sendTyping(channelId).catch(() => { /* Presence is best effort. */ });
            }
          }}
          onKeyDown={handleKeyDown}
          placeholder="Envoyer un message chiffré..."
          rows={1}
          className="flex-1 bg-transparent resize-none outline-none text-sm
                     text-liberte-text placeholder-liberte-muted py-2
                     max-h-32 overflow-y-auto"
        />

        <button
          onClick={handleSend}
          disabled={!content.trim() || sending}
          className={`p-2 rounded-lg transition-colors ${
            content.trim()
              ? "text-liberte-accent hover:bg-liberte-panel"
              : "text-liberte-muted cursor-not-allowed"
          }`}
        >
          <Send className="w-5 h-5" />
        </button>
      </div>

      <div className="flex justify-between mt-1 px-1">
        <span className="text-xs text-liberte-muted">
          E2EE XChaCha20-Poly1305
        </span>
        <span className="text-xs text-liberte-muted">
          {content.length}/{MAX_MESSAGE_LENGTH}
        </span>
      </div>
    </div>
  );
}
