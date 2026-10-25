import { useEffect, useState } from "react";
import { appDataDir } from "@tauri-apps/api/path";
import { useConfigStore } from "../stores/configStore";
import { Sun, Moon, Monitor } from "lucide-react";
import type { ThemeMode } from "../lib/types";
import { Eye } from "lucide-react";

export function GeneralSettings() {
  const theme = useConfigStore((s) => s.theme);
  const setTheme = useConfigStore((s) => s.setTheme);
  const startOnLogin = useConfigStore(s => s.startOnLogin);
  const setStartOnLogin = useConfigStore(s => s.setStartOnLogin);
  const autoSummary = useConfigStore((s) => s.autoSummary);
  const setAutoSummary = useConfigStore((s) => s.setAutoSummary);
  const [dataDirectory, setDataDirectory] = useState("");
  const overlayOpacity = useConfigStore((s) => s.overlayOpacity);
  const setOverlayOpacity = useConfigStore((s) => s.setOverlayOpacity);

  useEffect(() => {
    let active = true;
    void appDataDir().then(path => { if (active) setDataDirectory(path); }).catch(console.error);
    return () => { active = false; };
  }, []);

  const themeOptions: { value: ThemeMode; label: string; icon: React.ReactNode }[] = [
    { value: "dark", label: "Dark", icon: <Moon className="h-3.5 w-3.5" /> },
    { value: "light", label: "Light", icon: <Sun className="h-3.5 w-3.5" /> },
    {
      value: "system",
      label: "System",
      icon: <Monitor className="h-3.5 w-3.5" />,
    },
  ];

  return (
    <div className="space-y-6">
      {/* Theme Toggle */}
      <div className="rounded-xl border border-border/30 bg-card/50 p-5">
        <div className="flex items-center justify-between">
          <div>
            <label className="text-sm font-medium text-foreground">Theme</label>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Choose your preferred appearance
            </p>
          </div>
          <div className="flex rounded-lg border border-border/50 bg-secondary/30 p-0.5">
            {themeOptions.map((opt) => (
              <button
                key={opt.value}
                onClick={() => setTheme(opt.value)}
                className={`flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-all duration-150 cursor-pointer ${
                  theme === opt.value
                    ? "bg-primary text-primary-foreground shadow-sm shadow-primary/20"
                    : "text-muted-foreground hover:text-foreground hover:bg-secondary/50 active:scale-95"
                }`}
              >
                {opt.icon}
                {opt.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Toggle Options */}
      <div className="rounded-xl border border-border/30 bg-card/50 p-5 space-y-5">
        <h3 className="text-sm font-semibold text-primary/80">Behavior</h3>

        {/* Auto-Summary Toggle */}
        <div className="flex items-center justify-between">
          <div>
            <label className="text-sm font-medium text-foreground">
              Auto-Summary
            </label>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Generate meeting summary when meeting ends
            </p>
          </div>
          <button
            onClick={() => setAutoSummary(!autoSummary)}
            role="switch"
            aria-checked={autoSummary}
            aria-label="Toggle auto-summary"
            className={`relative h-6 w-11 shrink-0 cursor-pointer rounded-full transition-all duration-200 ${
              autoSummary ? "bg-primary shadow-[0_0_0_3px_hsl(var(--primary)/0.12)]" : "bg-muted"
            }`}
          >
            <span
              className={`absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white shadow-sm transition-all duration-200 ${
                autoSummary ? "translate-x-5 scale-[1.05]" : "translate-x-0"
              }`}
            />
          </button>
        </div>

        <div className="h-px bg-border/20" />

        {/* Start on Login Toggle */}
        <div className="flex items-center justify-between">
          <div>
            <label className="text-sm font-medium text-foreground">
              Start on Login
            </label>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Launch Kakehashi automatically when you log in
            </p>
          </div>
          <button
            onClick={() => setStartOnLogin(!startOnLogin)}
            role="switch"
            aria-checked={startOnLogin}
            aria-label="Toggle start on login"
            className={`relative h-6 w-11 shrink-0 cursor-pointer rounded-full transition-all duration-200 ${
              startOnLogin ? "bg-primary shadow-[0_0_0_3px_hsl(var(--primary)/0.12)]" : "bg-muted"
            }`}
          >
            <span
              className={`absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white shadow-sm transition-all duration-200 ${
                startOnLogin ? "translate-x-5 scale-[1.05]" : "translate-x-0"
              }`}
            />
          </button>
        </div>
      </div>


      {/* Moved notice */}
      <div className="rounded-xl border border-info/20 bg-info/5 px-5 py-3">
        <p className="text-xs text-info/80">
          Context Window, Auto-Trigger, and AI instructions have moved to the <strong className="text-info">AI Actions</strong> tab.
        </p>
      </div>

      {/* Overlay Opacity */}
      <div className="rounded-xl border border-border/30 bg-card/50 p-5">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Eye className="h-3.5 w-3.5 text-muted-foreground" />
            <div>
              <label className="text-sm font-medium text-foreground">Overlay Transparency</label>
              <p className="mt-0.5 text-xs text-muted-foreground">
                How transparent the overlay background is
              </p>
            </div>
          </div>
          <span className="text-xs font-mono text-muted-foreground w-10 text-right">
            {Math.round(overlayOpacity * 100)}%
          </span>
        </div>
        <input
          type="range"
          min={0.1}
          max={1}
          step={0.01}
          value={overlayOpacity}
          onChange={(e) => setOverlayOpacity(parseFloat(e.target.value))}
          className="w-full h-1.5 rounded-full appearance-none bg-secondary cursor-pointer accent-primary"
        />
        <div className="flex justify-between mt-1">
          <span className="text-[10px] text-muted-foreground/50">Transparent</span>
          <span className="text-[10px] text-muted-foreground/50">Solid</span>
        </div>
      </div>

      {/* Data Directory */}
      <div className="rounded-xl border border-border/30 bg-card/50 p-5">
        <div className="mb-3">
          <label className="text-sm font-medium text-foreground">
            Data Directory
          </label>
          <p className="mt-0.5 text-xs text-muted-foreground">
            Where meeting data and recordings are stored
          </p>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex-1 rounded-lg border border-border/50 bg-secondary/30 px-3.5 py-2.5">
            <p data-testid="app-data-directory" className="select-text break-all text-xs text-muted-foreground">
              {dataDirectory || "Default app data directory"}
            </p>
          </div>

        </div>
      </div>
    </div>
  );
}
