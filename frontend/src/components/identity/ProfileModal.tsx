import { useState } from "react";
import { X, Copy, Check, Pencil } from "lucide-react";
import type { UserStatus } from "../../lib/types";
import { useIdentityStore } from "../../stores/identityStore";

interface Props {
  isOpen: boolean;
  onClose: () => void;
  /** If provided, show another user's profile (read-only). Otherwise show own. */
  userId?: string;
  displayName?: string;
}

const STATUS_LABELS: Record<UserStatus, string> = {
  online: "En ligne",
  dnd: "Ne pas déranger",
  idle: "Inactif",
  invisible: "Invisible",
};

const STATUS_COLORS: Record<UserStatus, string> = {
  online: "bg-liberte-success",
  dnd: "bg-red-500",
  idle: "bg-yellow-500",
  invisible: "bg-gray-500",
};

export default function ProfileModal({
  isOpen,
  onClose,
  userId,
  displayName,
}: Props) {
  const { identity, setBio, setStatus, setDisplayName } = useIdentityStore();
  const [copied, setCopied] = useState(false);
  const [editingBio, setEditingBio] = useState(false);
  const [bioInput, setBioInput] = useState("");
  const [editingName, setEditingName] = useState(false);
  const [nameInput, setNameInput] = useState("");

  if (!isOpen) return null;

  const isOwnProfile = !userId || userId === identity?.publicKey;
  const profilePubkey = userId || identity?.publicKey || "";
  const profileName = isOwnProfile
    ? identity?.displayName
    : displayName;
  const profileBio = isOwnProfile ? identity?.bio : undefined;
  const profileStatus: UserStatus = isOwnProfile
    ? identity?.status || "online"
    : "online";
  const shortId = profilePubkey.length > 16
    ? `${profilePubkey.slice(0, 8)}…${profilePubkey.slice(-8)}`
    : profilePubkey;

  const avatarLetters = profileName
    ? profileName.slice(0, 2).toUpperCase()
    : shortId.slice(0, 2).toUpperCase();

  const copyPubkey = () => {
    navigator.clipboard.writeText(profilePubkey);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleSaveBio = async () => {
    await setBio(bioInput.trim());
    setEditingBio(false);
  };

  const handleSaveName = async () => {
    await setDisplayName(nameInput.trim());
    setEditingName(false);
  };

  const handleStatusChange = async (newStatus: UserStatus) => {
    await setStatus(newStatus);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-liberte-surface border border-liberte-border rounded-xl w-full max-w-md overflow-hidden shadow-2xl">
        {/* Banner */}
        <div
          className="h-24 relative"
          style={{
            background: `linear-gradient(135deg, hsl(${hashCode(profilePubkey) % 360}, 60%, 35%), hsl(${(hashCode(profilePubkey) + 60) % 360}, 60%, 25%))`,
          }}
        >
          <button
            onClick={onClose}
            className="absolute top-2 right-2 p-1.5 bg-black/30 hover:bg-black/50 rounded-full transition-colors"
          >
            <X className="w-4 h-4 text-white" />
          </button>
        </div>

        {/* Avatar overlapping banner */}
        <div className="relative px-4 -mt-10">
          <div className="relative inline-block">
            <div
              className="w-20 h-20 rounded-full flex items-center justify-center text-xl font-bold border-4 border-liberte-surface"
              style={{
                backgroundColor: `hsl(${hashCode(profilePubkey) % 360}, 60%, 40%)`,
              }}
            >
              {avatarLetters}
            </div>
            {/* Status dot */}
            <div
              className={`absolute bottom-1 right-1 w-4 h-4 rounded-full border-2 border-liberte-surface ${STATUS_COLORS[profileStatus]}`}
              title={STATUS_LABELS[profileStatus]}
            />
          </div>
        </div>

        {/* Profile info */}
        <div className="px-4 pt-2 pb-4 space-y-4">
          {/* Name */}
          <div>
            {isOwnProfile && editingName ? (
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  value={nameInput}
                  onChange={(e) => setNameInput(e.target.value.slice(0, 32))}
                  onKeyDown={(e) => e.key === "Enter" && handleSaveName()}
                  placeholder="Pseudo"
                  maxLength={32}
                  autoFocus
                  className="flex-1 bg-liberte-bg border border-liberte-border rounded px-2 py-1 text-sm outline-none focus:border-liberte-accent"
                />
                <button onClick={handleSaveName} className="p-1 hover:bg-liberte-panel rounded">
                  <Check className="w-4 h-4 text-liberte-success" />
                </button>
              </div>
            ) : (
              <div className="flex items-center gap-2">
                <h3 className="text-lg font-bold">
                  {profileName || shortId}
                </h3>
                {profileName && (
                  <span className="text-xs text-liberte-muted font-mono">
                    {shortId}
                  </span>
                )}
                {isOwnProfile && (
                  <button
                    onClick={() => {
                      setNameInput(profileName || "");
                      setEditingName(true);
                    }}
                    className="p-1 hover:bg-liberte-panel rounded"
                  >
                    <Pencil className="w-3.5 h-3.5 text-liberte-muted" />
                  </button>
                )}
              </div>
            )}
            <p className="text-xs text-liberte-muted flex items-center gap-1 mt-0.5">
              <span className={`w-2 h-2 rounded-full ${STATUS_COLORS[profileStatus]}`} />
              {STATUS_LABELS[profileStatus]}
            </p>
          </div>

          {/* Status selector (own profile only) */}
          {isOwnProfile && (
            <div className="bg-liberte-bg rounded-lg p-3 space-y-2">
              <label className="text-xs text-liberte-muted font-medium uppercase tracking-wider">
                Statut
              </label>
              <div className="grid grid-cols-2 gap-1.5">
                {(Object.keys(STATUS_LABELS) as UserStatus[]).map((s) => (
                  <button
                    key={s}
                    onClick={() => handleStatusChange(s)}
                    className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm transition-colors ${
                      profileStatus === s
                        ? "bg-liberte-panel text-liberte-text"
                        : "hover:bg-liberte-panel/50 text-liberte-muted"
                    }`}
                  >
                    <span className={`w-2.5 h-2.5 rounded-full ${STATUS_COLORS[s]}`} />
                    {STATUS_LABELS[s]}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Bio */}
          <div className="bg-liberte-bg rounded-lg p-3 space-y-2">
            <div className="flex items-center justify-between">
              <label className="text-xs text-liberte-muted font-medium uppercase tracking-wider">
                Bio
              </label>
              {isOwnProfile && !editingBio && (
                <button
                  onClick={() => {
                    setBioInput(profileBio || "");
                    setEditingBio(true);
                  }}
                  className="p-1 hover:bg-liberte-panel rounded"
                >
                  <Pencil className="w-3 h-3 text-liberte-muted" />
                </button>
              )}
            </div>
            {editingBio ? (
              <div className="space-y-2">
                <textarea
                  value={bioInput}
                  onChange={(e) => setBioInput(e.target.value.slice(0, 190))}
                  placeholder="Parlez de vous..."
                  maxLength={190}
                  rows={3}
                  autoFocus
                  className="w-full bg-liberte-surface border border-liberte-border rounded px-2 py-1.5 text-sm resize-none outline-none focus:border-liberte-accent"
                />
                <div className="flex items-center justify-between">
                  <span className="text-xs text-liberte-muted">
                    {bioInput.length}/190
                  </span>
                  <div className="flex gap-1">
                    <button
                      onClick={() => setEditingBio(false)}
                      className="px-2 py-1 text-xs text-liberte-muted hover:text-liberte-text"
                    >
                      Annuler
                    </button>
                    <button
                      onClick={handleSaveBio}
                      className="px-2 py-1 text-xs bg-liberte-accent rounded text-white hover:bg-opacity-90"
                    >
                      Sauvegarder
                    </button>
                  </div>
                </div>
              </div>
            ) : (
              <p className="text-sm text-liberte-text">
                {profileBio || (
                  <span className="text-liberte-muted italic">
                    {isOwnProfile ? "Ajoutez une bio..." : "Aucune bio"}
                  </span>
                )}
              </p>
            )}
          </div>

          {/* Public key */}
          <div className="bg-liberte-bg rounded-lg p-3 space-y-2">
            <label className="text-xs text-liberte-muted font-medium uppercase tracking-wider">
              Clé publique
            </label>
            <div className="flex items-center gap-2">
              <code className="flex-1 text-xs bg-liberte-surface p-2 rounded font-mono break-all text-liberte-muted">
                {profilePubkey}
              </code>
              <button
                onClick={copyPubkey}
                className="p-2 hover:bg-liberte-panel rounded shrink-0"
              >
                {copied ? (
                  <Check className="w-3.5 h-3.5 text-liberte-success" />
                ) : (
                  <Copy className="w-3.5 h-3.5 text-liberte-muted" />
                )}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function hashCode(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = (hash << 5) - hash + str.charCodeAt(i);
    hash |= 0;
  }
  return Math.abs(hash);
}
