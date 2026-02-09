import { X } from "lucide-react";
import { useEffect, useRef } from "react";

interface Props {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  children: React.ReactNode;
}

export default function Modal({ isOpen, onClose, title, children }: Props) {
  const overlayRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    if (isOpen) document.addEventListener("keydown", handleEsc);
    return () => document.removeEventListener("keydown", handleEsc);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div
      ref={overlayRef}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={(e) => {
        if (e.target === overlayRef.current) onClose();
      }}
    >
      <div className="panel p-0 max-w-md w-full mx-4 shadow-2xl">
        <div className="flex items-center justify-between p-4 border-b border-liberte-border">
          <h3 className="font-medium">{title}</h3>
          <button
            onClick={onClose}
            className="p-1 hover:bg-liberte-panel rounded transition-colors"
          >
            <X className="w-4 h-4 text-liberte-muted" />
          </button>
        </div>
        <div className="p-4">{children}</div>
      </div>
    </div>
  );
}
