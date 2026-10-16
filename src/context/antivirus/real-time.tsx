import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { useSettings } from "@/context/settings";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { getErrorMessage } from "@/lib/helpers";
import { store } from "@/lib/store";

type RealtimeContextValue = {
     enabled: boolean;
     start(): Promise<void>;
     stop(): Promise<void>;
};

const RealtimeContext = createContext<RealtimeContextValue | null>(null);

export function RealtimeProvider({ children }: { children: React.ReactNode }) {
     const { settings } = useSettings();
     const {t: messageTxt} = useTranslation("messages");
     const [enabled, setEnabled] = useState(settings.realTime);
     const start = useCallback(async () => {
          await invoke("start_real_time_scan", { behavior: settings.behavior || "balanced" });
          await invoke("update_tray_icon",{state: "enabled"})
     },[settings.behavior]);
     const stop = useCallback(async () => {
          await invoke("stop_real_time_scan");
          await invoke("update_tray_icon",{state: "disabled"})
     },[]);
     const isFirstRun = useRef(true);
     useEffect(() => {
          const firstRun = isFirstRun.current;
          isFirstRun.current = false;
          const behavior = settings.behavior || "balanced";
          // The backend starts protection itself at launch (a login launch never builds this window),
          // so it reads these preferences from the store.
          store.set("realTime", settings.realTime).catch(() => {});
          store.set("behavior", behavior).catch(() => {});
          let cancelled = false;
          let retry: ReturnType<typeof setTimeout> | undefined;
          // Only report protection as on when the backend actually started it. Protection is on by default,
          // so a failed start (e.g. during login) is retried instead of leaving the PC unprotected.
          const attempt = (notify: boolean) => {
               start().then(() => { if (!cancelled) setEnabled(true) }).catch(e => {
                    if (cancelled) return;
                    setEnabled(false);
                    invoke("update_tray_icon",{state: "disabled"}).catch(()=>{});
                    if (notify) toast.error(messageTxt("realtime-start-error"), { description: getErrorMessage(e) });
                    retry = setTimeout(() => attempt(false), 30_000);
               });
          };
          (async () => {
               if (firstRun) {
                    // The window is rebuilt every time it opens: adopt the running session instead of restarting it.
                    const status = await invoke<{running: boolean, behavior: string | null}>("get_real_time_status").catch(() => null);
                    if (cancelled) return;
                    if (settings.realTime && status?.running && status.behavior === behavior) {
                         setEnabled(true);
                         return;
                    }
                    if (!settings.realTime) {
                         // Nothing to stop unless the backend started it; stopping would only log a bogus entry.
                         if (status?.running) stop().catch(console.error);
                         setEnabled(false);
                         return;
                    }
               } else if (!settings.realTime) {
                    stop().catch(console.error);
                    setEnabled(false);
                    return;
               }
               attempt(true);
          })();
          return () => { cancelled = true; clearTimeout(retry); };
     }, [settings.realTime, settings.behavior]);
     const values: RealtimeContextValue = useMemo(()=>({
          enabled,
          start: async () => {
               await start();
               setEnabled(true);
          },
          stop: async () => {
               await stop();
               setEnabled(false);
          },
     }),[enabled])
     return (
          <RealtimeContext.Provider
               value={values}
          >
               {children}
          </RealtimeContext.Provider>
     );
}

export function useRealtimeScan() {
     const ctx = useContext(RealtimeContext);
     if (!ctx) {
          throw new Error("useRealtimeContext must be used inside RealtimeProvider");
     }
     return ctx;
}