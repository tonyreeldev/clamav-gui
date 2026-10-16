import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { BrickWall, ExternalLink, FolderLock, Globe, RotateCw, ScanEye, ShieldCheck, Terminal, type LucideIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import CommandSnippetBlock from "@/components/command-snippet";
import { getErrorMessage } from "@/lib/helpers";
import { cn } from "@/lib/utils";

// Mirrors src-tauri/src/system/security.rs
type SecurityCheck = "firewall" | "antivirus" | "network-protection" | "ransomware-protection" | "smart-screen";
type SecurityState = "on" | "off" | "partial" | "audit" | "third-party" | "outdated" | "not-applicable" | "unknown";
interface SecurityItem {
     check: SecurityCheck,
     state: SecurityState,
     detail: string | null
}

const ICONS: Record<SecurityCheck, LucideIcon> = {
     firewall: BrickWall,
     antivirus: ShieldCheck,
     "network-protection": Globe,
     "ransomware-protection": FolderLock,
     "smart-screen": ScanEye,
};
const TONE: Record<SecurityState, string> = {
     on: "bg-emerald-500",
     "third-party": "bg-emerald-500",
     off: "bg-red-500",
     partial: "bg-amber-500",
     audit: "bg-amber-500",
     outdated: "bg-amber-500",
     "not-applicable": "bg-muted-foreground/50",
     unknown: "bg-muted-foreground/50",
};
const ENABLE_NETWORK_PROTECTION = "Set-MpPreference -EnableNetworkProtection Enabled";

// Read-only view of the Windows protections ClamAV doesn't provide; actions only open Windows Security.
export default function WindowsSecurity(){
     const {t} = useTranslation("overview", {keyPrefix: "windows-security"});
     const [items, setItems] = useState<SecurityItem[] | null>(null);
     const [error, setError] = useState(false);
     const [isLoading, setIsLoading] = useState(true);
     const [showCommand, setShowCommand] = useState(false);

     // The query runs PowerShell for a few seconds: never two at once, and focus changes re-check at most every 15 s.
     const inFlight = useRef(false);
     const lastRun = useRef(0);
     const refresh = useCallback(async () => {
          if (inFlight.current) return;
          inFlight.current = true;
          lastRun.current = Date.now();
          setIsLoading(true);
          try {
               setItems(await invoke<SecurityItem[]>("get_security_status"));
               setError(false);
          } catch {
               setError(true);
          } finally {
               inFlight.current = false;
               setIsLoading(false);
          }
     }, []);
     useEffect(() => {
          refresh();
          // The user usually comes back from Windows Security after changing something.
          const onFocus = () => { if (Date.now() - lastRun.current > 15_000) refresh(); };
          window.addEventListener("focus", onFocus);
          return () => window.removeEventListener("focus", onFocus);
     }, [refresh]);

     const openSettings = async (check: SecurityCheck) => {
          try {
               await invoke("open_security_settings", {check});
          } catch (err) {
               toast.error(t("error"), {description: getErrorMessage(err)});
          }
     };
     const detailText = ({check, state, detail}: SecurityItem): string | null => {
          if (state === "third-party") return detail ? t("details.managed-by", {name: detail}) : null;
          if (state === "outdated") return t("details.signatures-age", {count: Number(detail) || 0});
          if (state === "audit") return t("details.audit");
          if (state === "not-applicable") return t("details.not-applicable");
          if (state === "unknown") return t("details.unknown");
          if (check === "firewall" && (state === "off" || state === "partial") && detail) {
               const profiles = detail.split(", ").map(p => t(`profiles.${p as "Domain" | "Private" | "Public"}`, {defaultValue: p}));
               return t("details.profiles-off", {profiles: profiles.join(", ")});
          }
          if (check === "smart-screen" && state === "partial" && detail) return t("details.off-in", {where: detail});
          return null;
     };

     // Non-Windows: the backend returns nothing and there is nothing to show.
     if (items && items.length === 0 && !error) return null;
     return (
          <section className="w-full px-2 pb-4 space-y-3" aria-busy={isLoading}>
               <div className="flex items-center justify-between gap-2">
                    <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t("title")}</h2>
                    <Button variant="ghost" size="sm" onClick={refresh} disabled={isLoading}>
                         <RotateCw className={cn(isLoading && "animate-spin")}/> {t("refresh")}
                    </Button>
               </div>
               <p className="text-sm text-muted-foreground">{t("intro")}</p>
               {error && !items ? (
                    <div className="rounded-xl border bg-card p-4 text-sm flex items-center justify-between gap-3">
                         <span>{t("error")}</span>
                         <Button variant="outline" size="sm" onClick={refresh}><RotateCw/> {t("refresh")}</Button>
                    </div>
               ) : (
                    <ul className="rounded-xl border bg-card divide-y">
                         {!items ? Array.from({length: 5}, (_, i) => (
                              <li key={i} className="p-4 flex items-center gap-3">
                                   <Skeleton className="size-9 rounded-lg"/>
                                   <div className="flex-1 space-y-2"><Skeleton className="h-4 w-40"/><Skeleton className="h-3 w-64 max-w-full"/></div>
                              </li>
                         )) : items.map(item => {
                              const Icon = ICONS[item.check];
                              const detail = detailText(item);
                              const isNetwork = item.check === "network-protection";
                              const canEnableNetwork = isNetwork && ["off", "audit"].includes(item.state);
                              return (
                                   <li key={item.check} className="p-4 space-y-3">
                                        <div className="flex items-start gap-3">
                                             <div className="size-9 shrink-0 rounded-lg bg-muted flex items-center justify-center"><Icon className="size-5"/></div>
                                             <div className="flex-1 min-w-0">
                                                  <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                                                       <span className="font-medium">{t(`checks.${item.check}.name`)}</span>
                                                       <span className="flex items-center gap-1.5 text-sm">
                                                            <span aria-hidden className={cn("size-2 rounded-full", TONE[item.state])}/>
                                                            {t(`states.${item.state}`)}
                                                       </span>
                                                  </div>
                                                  <p className="text-sm text-muted-foreground">{t(`checks.${item.check}.desc`)}</p>
                                                  {detail && <p className="text-sm">{detail}</p>}
                                             </div>
                                             {isNetwork ? canEnableNetwork && (
                                                  <Button variant="outline" size="sm" className="shrink-0" onClick={() => setShowCommand(v => !v)} aria-expanded={showCommand}>
                                                       <Terminal/> {t("how-to-enable")}
                                                  </Button>
                                             ) : (
                                                  <Button variant="outline" size="sm" className="shrink-0" onClick={() => openSettings(item.check)}>
                                                       <ExternalLink/> {t("open")}
                                                  </Button>
                                             )}
                                        </div>
                                        {canEnableNetwork && showCommand && (
                                             <div className="space-y-2 sm:pl-12">
                                                  <p className="text-sm text-muted-foreground">{t("command-note")}</p>
                                                  <CommandSnippetBlock command={ENABLE_NETWORK_PROTECTION}/>
                                             </div>
                                        )}
                                   </li>
                              );
                         })}
                    </ul>
               )}
          </section>
     )
}
