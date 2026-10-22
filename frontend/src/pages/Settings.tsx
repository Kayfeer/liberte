import { useIdentityStore } from "../stores/identityStore";
import { useNavigationStore } from "../stores/navigationStore";
import { useMediaDevices } from "../hooks/useMediaDevices";
import { useThemeStore, type ThemeName } from "../stores/themeStore";
import {
  Shield,
  Copy,
  Check,
  ArrowLeft,
  Mic,
  Volume2,
  Video,
  AudioLines,
  RefreshCw,
  Palette,
  Pencil,
} from "lucide-react";
import { useState, useRef, useEffect } from "react";
import NetworkSettings from "../components/settings/NetworkSettings";
import { ArchiveExport, BackupSettings } from "../components/settings/ArchiveControls";
import PremiumSettings from "../components/settings/PremiumSettings";
import IceSettings from "../components/settings/IceSettings";

/* ─── Reusable sub-components ─── */

function DeviceSelect({
  label,
  icon: Icon,
  devices,
  value,
  onChange,
}: {
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  devices: { deviceId: string; label: string }[];
  value: string;
  onChange: (id: string) => void;
}) {
  return (
    <div className="space-y-1.5">
      <label className="text-xs text-liberte-muted flex items-center gap-1.5">
        <Icon className="w-3.5 h-3.5" />
        {label}
      </label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="input-field text-sm"
      >
        {devices.length === 0 && (
          <option value="">Aucun périphérique détecté</option>
        )}
        {devices.map((d) => (
          <option key={d.deviceId} value={d.deviceId}>
            {d.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function VolumeSlider({
  label,
  value,
  onChange,
  max = 200,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  max?: number;
}) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between">
        <label className="text-xs text-liberte-muted">{label}</label>
        <span className="text-xs font-mono text-liberte-muted">{value}%</span>
      </div>
      <input
        type="range"
        min={0}
        max={max}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-full h-1.5 bg-liberte-border rounded-full appearance-none cursor-pointer
                   accent-liberte-accent"
      />
    </div>
  );
}

function Toggle({
  label,
  description,
  checked,
  onChange,
  badge,
}: {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  badge?: string;
}) {
  return (
    <label className="flex items-start gap-3 cursor-pointer group">
      <input
        type="checkbox"
        className="sr-only"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <div className="pt-0.5">
        <div
          className={`w-9 h-5 rounded-full transition-colors duration-200 flex items-center px-0.5 ${
            checked ? "bg-liberte-accent" : "bg-liberte-border"
          }`}
        >
          <div
            className={`w-4 h-4 bg-white rounded-full shadow-sm transition-transform duration-200 ${
              checked ? "translate-x-4" : "translate-x-0"
            }`}
          />
        </div>
      </div>
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium group-hover:text-liberte-accent transition-colors">
            {label}
          </span>
          {badge && (
            <span className="text-[10px] bg-liberte-accent/20 text-liberte-accent px-1.5 py-0.5 rounded-full font-medium">
              {badge}
            </span>
          )}
        </div>
        {description && (
          <p className="text-xs text-liberte-muted mt-0.5">{description}</p>
        )}
      </div>
    </label>
  );
}

/* ─── Main Settings page ─── */

function CameraPreview({ deviceId }: { deviceId: string }) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    let stream: MediaStream | null = null;
    setError(null);
    if (deviceId) {
      void navigator.mediaDevices.getUserMedia({
        video: { deviceId: { exact: deviceId }, width: 640, height: 360 }, audio: false,
      }).then((captured) => {
        if (cancelled) { captured.getTracks().forEach((track) => track.stop()); return; }
        stream = captured;
        if (videoRef.current) videoRef.current.srcObject = captured;
      }).catch((cause) => { if (!cancelled) setError(String(cause)); });
    }
    return () => {
      cancelled = true;
      stream?.getTracks().forEach((track) => track.stop());
    };
  }, [deviceId]);
  return <div className="space-y-2">
    <video ref={videoRef} autoPlay playsInline muted className="w-full rounded-lg bg-liberte-bg aspect-video object-cover" />
    {error && <p role="alert" className="text-xs text-red-300">{error}</p>}
  </div>;
}

export default function Settings() {
  const { identity, setDisplayName, setBio } = useIdentityStore();
  const navigate = useNavigationStore((s) => s.navigate);
  const media = useMediaDevices();
  const [copied, setCopied] = useState(false);
  const [editingName, setEditingName] = useState(false);
  const [nameInput, setNameInput] = useState("");
  const [editingBio, setEditingBio] = useState(false);
  const [bioInput, setBioInput] = useState("");

  const copyPubkey = () => {
    if (identity) {
      navigator.clipboard.writeText(identity.publicKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleSaveName = async () => {
    await setDisplayName(nameInput.trim());
    setEditingName(false);
  };

  return (
    <div className="p-6 max-w-2xl mx-auto space-y-6 overflow-y-auto flex-1">
      {/* ── Header ── */}
      <div className="flex items-center gap-3">
        <button
          onClick={() => navigate("home")}
          className="p-1.5 hover:bg-liberte-panel rounded transition-colors"
        >
          <ArrowLeft className="w-5 h-5 text-liberte-muted" />
        </button>
        <h2 className="text-xl font-bold">Paramètres</h2>
      </div>

      <NetworkSettings />

      <ThemeSection />

      {/* ── Identity ── */}
      <div className="panel p-4 space-y-4">
        <h3 className="font-medium flex items-center gap-2">
          <Shield className="w-4 h-4 text-liberte-accent" />
          Identité
        </h3>

        {identity && (
          <div className="space-y-2">
            {/* Display name */}
            <div>
              <label className="text-xs text-liberte-muted">Pseudo</label>
              {editingName ? (
                <div className="flex items-center gap-2 mt-1">
                  <input
                    type="text"
                    value={nameInput}
                    onChange={(e) => setNameInput(e.target.value.slice(0, 32))}
                    onKeyDown={(e) => e.key === "Enter" && handleSaveName()}
                    placeholder="Ex: Kayfeer"
                    maxLength={32}
                    autoFocus
                    className="flex-1 bg-liberte-bg border border-liberte-border rounded px-2 py-1
                               text-sm text-liberte-text placeholder-liberte-muted outline-hidden
                               focus:border-liberte-accent transition-colors"
                  />
                  <button
                    onClick={handleSaveName}
                    className="p-1.5 hover:bg-liberte-panel rounded transition-colors"
                  >
                    <Check className="w-4 h-4 text-liberte-success" />
                  </button>
                </div>
              ) : (
                <div className="flex items-center gap-2 mt-1">
                  <p className="text-sm font-medium">
                    {identity.displayName || (
                      <span className="text-liberte-muted italic">Aucun pseudo défini</span>
                    )}
                  </p>
                  <button
                    onClick={() => {
                      setNameInput(identity.displayName || "");
                      setEditingName(true);
                    }}
                    className="p-1 hover:bg-liberte-panel rounded transition-colors"
                  >
                    <Pencil className="w-3.5 h-3.5 text-liberte-muted" />
                  </button>
                </div>
              )}
            </div>

            <div>
              <label className="text-xs text-liberte-muted">
                Clé publique
              </label>
              <div className="flex items-center gap-2 mt-1">
                <code className="flex-1 text-xs bg-liberte-bg p-2 rounded font-mono break-all">
                  {identity.publicKey}
                </code>
                <button
                  onClick={copyPubkey}
                  className="p-2 hover:bg-liberte-panel rounded transition-colors"
                >
                  {copied ? (
                    <Check className="w-4 h-4 text-liberte-success" />
                  ) : (
                    <Copy className="w-4 h-4 text-liberte-muted" />
                  )}
                </button>
              </div>
            </div>

            <div>
              <label className="text-xs text-liberte-muted">ID court</label>
              <p className="text-sm font-mono mt-1">{identity.shortId}</p>
            </div>

            {/* Bio */}
            <div>
              <div className="flex items-center justify-between">
                <label className="text-xs text-liberte-muted">Bio</label>
                {!editingBio && (
                  <button
                    onClick={() => {
                      setBioInput(identity.bio || "");
                      setEditingBio(true);
                    }}
                    className="p-1 hover:bg-liberte-panel rounded transition-colors"
                  >
                    <Pencil className="w-3 h-3 text-liberte-muted" />
                  </button>
                )}
              </div>
              {editingBio ? (
                <div className="mt-1 space-y-2">
                  <textarea
                    value={bioInput}
                    onChange={(e) => setBioInput(e.target.value.slice(0, 190))}
                    placeholder="Parlez de vous..."
                    maxLength={190}
                    rows={3}
                    autoFocus
                    className="w-full bg-liberte-bg border border-liberte-border rounded px-2 py-1.5
                               text-sm text-liberte-text placeholder-liberte-muted outline-hidden
                               focus:border-liberte-accent transition-colors resize-none"
                  />
                  <div className="flex items-center justify-between">
                    <span className="text-xs text-liberte-muted">{bioInput.length}/190</span>
                    <div className="flex gap-1">
                      <button
                        onClick={() => setEditingBio(false)}
                        className="px-2 py-1 text-xs text-liberte-muted hover:text-liberte-text"
                      >
                        Annuler
                      </button>
                      <button
                        onClick={async () => {
                          await setBio(bioInput.trim());
                          setEditingBio(false);
                        }}
                        className="px-2 py-1 text-xs bg-liberte-accent rounded text-white hover:bg-liberte-accent/90"
                      >
                        Sauvegarder
                      </button>
                    </div>
                  </div>
                </div>
              ) : (
                <p className="text-sm mt-1">
                  {identity.bio || (
                    <span className="text-liberte-muted italic">Aucune bio définie</span>
                  )}
                </p>
              )}
            </div>

            <div className="pt-3 border-t border-liberte-border"><ArchiveExport kind="profile" /></div>
          </div>
        )}
      </div>

      {/* ── Audio ── */}
      <div className="panel p-4 space-y-5">
        <div className="flex items-center justify-between">
          <h3 className="font-medium flex items-center gap-2">
            <Mic className="w-4 h-4 text-liberte-accent" />
            Audio
          </h3>
          <button
            onClick={media.refresh}
            className="p-1.5 hover:bg-liberte-panel rounded transition-colors"
            title="Rafraîchir les périphériques"
          >
            <RefreshCw className="w-3.5 h-3.5 text-liberte-muted" />
          </button>
        </div>

        {media.error && (
          <p className="text-xs text-liberte-accent bg-liberte-accent/10 p-2 rounded">
            ⚠ {media.error}
          </p>
        )}

        {/* Input device */}
        <DeviceSelect
          label="Périphérique d'entrée (micro)"
          icon={Mic}
          devices={media.audioInputs}
          value={media.selectedAudioInput}
          onChange={(id) => media.update({ selectedAudioInput: id })}
        />

        <VolumeSlider
          label="Volume d'entrée"
          value={media.inputVolume}
          onChange={(v) => media.update({ inputVolume: v })}
        />

        {/* Output device */}
        <DeviceSelect
          label="Périphérique de sortie (haut-parleurs / casque)"
          icon={Volume2}
          devices={media.audioOutputs}
          value={media.selectedAudioOutput}
          onChange={(id) => media.update({ selectedAudioOutput: id })}
        />

        <VolumeSlider
          label="Volume de sortie"
          value={Math.min(100, media.outputVolume)}
          max={100}
          onChange={(v) => media.update({ outputVolume: v })}
        />

        {/* Separator */}
        <div className="border-t border-liberte-border" />

        {/* Voice processing */}
        <h4 className="text-sm font-medium flex items-center gap-2">
          <AudioLines className="w-4 h-4 text-liberte-accent" />
          Traitement vocal
        </h4>

        <div className="space-y-4">
          <Toggle
            label="Suppression du bruit"
            description="Filtre les bruits de fond (clavier, ventilateur, environnement)"
            checked={media.noiseSuppression}
            onChange={(v) => media.update({ noiseSuppression: v })}
          />

          <Toggle
            label="Annulation d'écho"
            description="Empêche les retours audio entre le micro et les haut-parleurs"
            checked={media.echoCancellation}
            onChange={(v) => media.update({ echoCancellation: v })}
          />

          <Toggle
            label="Contrôle automatique du gain"
            description="Ajuste automatiquement le volume de votre micro pour un niveau constant"
            checked={media.autoGainControl}
            onChange={(v) => media.update({ autoGainControl: v })}
          />

          <Toggle
            label="Isolation vocale"
            description="Active le traitement d'isolation vocale du navigateur — idéal en environnement bruyant"
            checked={media.voiceIsolation}
            onChange={(v) => media.update({ voiceIsolation: v })}
            badge="Expérimental"
          />
        </div>
      </div>

      {/* ── Video ── */}
      <div className="panel p-4 space-y-5">
        <h3 className="font-medium flex items-center gap-2">
          <Video className="w-4 h-4 text-liberte-accent" />
          Vidéo
        </h3>

        <DeviceSelect
          label="Caméra"
          icon={Video}
          devices={media.videoInputs}
          value={media.selectedVideoInput}
          onChange={(id) => media.update({ selectedVideoInput: id })}
        />

        {media.videoInputs.length > 0 && (
          <CameraPreview deviceId={media.selectedVideoInput} />
        )}

        {media.videoInputs.length === 0 && !media.loading && (
          <p className="text-xs text-liberte-muted">
            Aucune caméra détectée. Branchez une webcam et cliquez sur
            rafraîchir.
          </p>
        )}
      </div>

      <BackupSettings />
      <IceSettings servers={media.iceServers} onSave={(iceServers) => media.update({ iceServers })} />
      <PremiumSettings />
    </div>
  );
}

/* ─── Theme Section ─── */

const THEME_OPTIONS: { id: ThemeName; label: string; preview: string[] }[] = [
  { id: "dark", label: "Sombre", preview: ["#1a1a2e", "#16213e", "#e94560"] },
  { id: "light", label: "Clair", preview: ["#f5f5f5", "#ffffff", "#e94560"] },
  { id: "midnight", label: "Minuit", preview: ["#0d0d1a", "#111127", "#8b5cf6"] },
  { id: "custom", label: "Personnalisé", preview: [] },
];

function ThemeSection() {
  const { themeName, setTheme, colors, setCustomColor } = useThemeStore();

  return (
    <div className="panel p-4 space-y-4">
      <h3 className="font-medium flex items-center gap-2">
        <Palette className="w-4 h-4 text-liberte-accent" />
        Thème
      </h3>

      <div className="grid grid-cols-4 gap-2">
        {THEME_OPTIONS.map((opt) => (
          <button
            key={opt.id}
            onClick={() => setTheme(opt.id)}
            className={`flex flex-col items-center gap-1.5 p-2 rounded-lg border transition-colors ${
              themeName === opt.id
                ? "border-liberte-accent bg-liberte-accent/10"
                : "border-liberte-border hover:border-liberte-muted"
            }`}
          >
            {opt.preview.length > 0 ? (
              <div className="flex gap-0.5">
                {opt.preview.map((c, i) => (
                  <div
                    key={i}
                    className="w-4 h-4 rounded-xs"
                    style={{ backgroundColor: c }}
                  />
                ))}
              </div>
            ) : (
              <div className="flex gap-0.5">
                <div className="w-4 h-4 rounded-xs bg-gradient-to-br from-purple-500 to-pink-500" />
                <div className="w-4 h-4 rounded-xs bg-gradient-to-br from-blue-500 to-green-500" />
                <div className="w-4 h-4 rounded-xs bg-gradient-to-br from-yellow-500 to-red-500" />
              </div>
            )}
            <span className="text-xs">{opt.label}</span>
          </button>
        ))}
      </div>

      {themeName === "custom" && (
        <div className="grid grid-cols-3 gap-3 pt-2">
          {(
            Object.keys(colors) as Array<keyof typeof colors>
          ).map((key) => (
            <label key={key} className="flex items-center gap-2 text-xs">
              <input
                type="color"
                value={colors[key]}
                onChange={(e) => setCustomColor(key, e.target.value)}
                className="w-6 h-6 rounded border border-liberte-border cursor-pointer"
              />
              <span className="text-liberte-muted">{key}</span>
            </label>
          ))}
        </div>
      )}
    </div>
  );
}
