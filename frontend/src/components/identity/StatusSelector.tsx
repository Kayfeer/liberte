import { useState, useRef, useEffect } from "react";
import type { UserStatus } from "../../lib/types";
import { useIdentityStore } from "../../stores/identityStore";

const STATUS_CONFIG: Record<
  UserStatus,
  { label: string; color: string; description: string }
> = {
  online: {
    label: "En ligne",
    color: "bg-liberte-success",
    description: "Vous êtes visible",
  },
  idle: {
    label: "Inactif",
    color: "bg-yellow-500",
    description: "Apparaître comme inactif",
  },
  dnd: {
    label: "Ne pas déranger",
    color: "bg-red-500",
    description: "Notifications silencieuses",
  },
  invisible: {
    label: "Invisible",
    color: "bg-gray-500",
    description: "Apparaître hors ligne",
  },
};

export default function StatusSelector() {
  const { identity, setStatus } = useIdentityStore();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const currentStatus: UserStatus = identity?.status || "online";

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 px-2 py-1 rounded hover:bg-liberte-panel transition-colors"
        title={STATUS_CONFIG[currentStatus].label}
      >
        <span
          className={`w-2.5 h-2.5 rounded-full ${STATUS_CONFIG[currentStatus].color}`}
        />
        <span className="text-xs text-liberte-muted">
          {STATUS_CONFIG[currentStatus].label}
        </span>
      </button>

      {open && (
        <div className="absolute bottom-full left-0 mb-1 w-56 bg-liberte-surface border border-liberte-border rounded-lg shadow-xl z-50 py-1">
          {(Object.keys(STATUS_CONFIG) as UserStatus[]).map((s) => {
            const cfg = STATUS_CONFIG[s];
            return (
              <button
                key={s}
                onClick={() => {
                  setStatus(s);
                  setOpen(false);
                }}
                className={`w-full flex items-center gap-3 px-3 py-2 text-sm hover:bg-liberte-panel transition-colors ${
                  currentStatus === s ? "bg-liberte-panel/50" : ""
                }`}
              >
                <span className={`w-3 h-3 rounded-full ${cfg.color}`} />
                <div className="text-left">
                  <div className="font-medium">{cfg.label}</div>
                  <div className="text-xs text-liberte-muted">
                    {cfg.description}
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
