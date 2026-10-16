import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { BrickWall, ExternalLink, FolderLock, Globe, RotateCw, ScanEye, ShieldAlert, ShieldCheck, type LucideIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Spinner } from "@/components/ui/spinner";
import AlertBox from "@/components/popup/alert";
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
interface PendingChange {
     check: SecurityCheck,
     enable: boolean
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
// The backend can only switch these; Defender's antivirus (Tamper Protection) and SmartScreen open their page instead.
const SWITCHABLE: SecurityCheck[] = ["firewall", "network-protection", "ransomware-protection"];
// What "Turn on recommended" enables. Ransomware protection is left out: it can block legitimate apps.
const RECOMMENDED: SecurityCheck[] = ["firewall", "network-protection"];

const canSwitch = (item: SecurityItem) =>
     SWITCHABLE.includes(item.check) && !["third-party", "not-applicable", "unknown"].includes(item.state);

// Windows protections ClamAV doesn't provide. Switches change them through one UAC prompt each.
export default function WindowsSecurity(){
     const {t} = useTranslation("overview", {keyPrefix: "windows-security"});
     const [items, setItems] = useState<SecurityItem[] | null>(null);
     const [error, setError] = useState(false);
     const [isLoading, setIsLoading] = useState(true);
     const [busy, setBusy] = useState<SecurityCheck[]>([]);
     const [confirm, setConfirm] = useState<PendingChange | null>(null);

     // The query runs PowerShell for a few seconds: never two at once, and focus changes re-check at most every 15 s.
     const inFlight = useRef(false);
     const lastRun = useRef(0);
     const refresh = useCallback(async (): Promise<SecurityItem[] | null> => {
          if (inFlight.current) return null;
          inFlight.current = true;
          lastRun.current = Date.now();
          setIsLoading(true);
          try {
               const result = await invoke<SecurityItem[]>("get_security_status");
               setItems(result);
               setError(false);
               return result;
          } catch {
               setError(true);
               return null;
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

     const name = (check: SecurityCheck) => t(`checks.${check}.name`);
     const apply = async (checks: SecurityCheck[], enable: boolean) => {
          setBusy(checks);
          try {
               await invoke("set_security_features", {checks, enable});
               // Re-read instead of trusting the exit code: a policy or another product can silently keep the old value.
               const fresh = await refresh();
               const applied = fresh?.filter(i => checks.includes(i.check)).every(i => (i.state === "on") === enable);
               if (applied === false) toast.warning(t("toggle.unchanged"));
               else toast.success(t("toggle.applied"));
          } catch (err) {
               const message = getErrorMessage(err);
               if (message === "UAC_CANCELLED") toast.info(t("toggle.cancelled"));
               else toast.error(t("toggle.failed"), {description: message});
          } finally {
               setBusy([]);
          }
     };
     // Turning something off, or on ransomware protection (it can block apps), asks first; the rest applies directly.
     const onToggle = (check: SecurityCheck, enable: boolean) => {
          if (!enable || check === "ransomware-protection") setConfirm({check, enable});
          else apply([check], enable);
     };
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
     const recommendedOff = (items ?? []).filter(i => RECOMMENDED.includes(i.check) && canSwitch(i) && i.state !== "on").map(i => i.check);
     const isBusy = busy.length > 0;
     return (
          <section className="w-full px-2 pb-4 space-y-3" aria-busy={isLoading}>
               <div className="flex items-center justify-between gap-2">
                    <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t("title")}</h2>
                    <Button variant="ghost" size="sm" onClick={refresh} disabled={isLoading || isBusy}>
                         <RotateCw className={cn(isLoading && "animate-spin")}/> {t("refresh")}
                    </Button>
               </div>
               <p className="text-sm text-muted-foreground">{t("intro")} {t("uac-note")}</p>
               {recommendedOff.length > 0 && (
                    <div className="rounded-xl border border-amber-500/40 bg-amber-500/10 p-4 flex flex-col sm:flex-row sm:items-center gap-3">
                         <ShieldAlert className="size-6 shrink-0 text-amber-600 dark:text-amber-400"/>
                         <div className="flex-1 min-w-0">
                              <p className="font-medium">{t("recommended.title")}</p>
                              <p className="text-sm text-muted-foreground">{t("recommended.desc")}</p>
                         </div>
                         <Button className="shrink-0" onClick={() => apply(recommendedOff, true)} disabled={isBusy}>
                              {isBusy ? <Spinner/> : <ShieldCheck/>} {t("recommended.button")}
                         </Button>
                    </div>
               )}
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
                              const switchable = canSwitch(item);
                              const isApplying = busy.includes(item.check);
                              return (
                                   <li key={item.check} className="p-4 flex items-start gap-3">
                                        <div className="size-9 shrink-0 rounded-lg bg-muted flex items-center justify-center"><Icon className="size-5"/></div>
                                        <div className="flex-1 min-w-0">
                                             <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                                                  <span className="font-medium">{name(item.check)}</span>
                                                  <span className="flex items-center gap-1.5 text-sm">
                                                       <span aria-hidden className={cn("size-2 rounded-full", TONE[item.state])}/>
                                                       {t(`states.${item.state}`)}
                                                  </span>
                                             </div>
                                             <p className="text-sm text-muted-foreground">{t(`checks.${item.check}.desc`)}</p>
                                             {detail && <p className="text-sm">{detail}</p>}
                                        </div>
                                        <div className="shrink-0 flex items-center gap-2 self-center">
                                             {isApplying && <Spinner className="size-4"/>}
                                             {switchable ? (
                                                  <Switch
                                                       checked={item.state === "on"}
                                                       onCheckedChange={enable => onToggle(item.check, enable)}
                                                       disabled={isBusy || isLoading}
                                                       aria-label={t("toggle.label", {name: name(item.check)})}
                                                  />
                                             ) : item.check !== "network-protection" && item.state !== "not-applicable" && (
                                                  <Button variant="outline" size="sm" onClick={() => openSettings(item.check)}>
                                                       <ExternalLink/> {t("open")}
                                                  </Button>
                                             )}
                                        </div>
                                   </li>
                              );
                         })}
                    </ul>
               )}
               <AlertBox
                    open={confirm !== null}
                    setOpen={open => { if (!open) setConfirm(null); }}
                    title={confirm?.enable ? t("confirm.cfa-title") : t("confirm.off-title", {name: confirm ? name(confirm.check) : ""})}
                    description={confirm?.enable ? t("confirm.cfa-desc") : t("confirm.off-desc")}
                    submitText={confirm?.enable ? t("confirm.cfa-submit") : t("confirm.off-submit")}
                    cancelText={t("confirm.cancel")}
                    type={confirm?.enable ? "default" : "danger"}
                    submitEvent={() => {
                         if (confirm) apply([confirm.check], confirm.enable);
                         setConfirm(null);
                    }}
               />
          </section>
     )
}
