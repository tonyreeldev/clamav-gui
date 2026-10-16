import { Button } from "@/components/ui/button";
import { INITIAL_DEF_UPDATE_STATE } from "@/lib/constants/states";
import { IDefsUpdaterState } from "@/lib/types/states";
import { cn } from "@/lib/utils";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { AlertCircle, BugOff, CheckCircle, RotateCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { formatDistanceToNow } from "date-fns";
import { Spinner } from "@/components/ui/spinner";
import { invoke } from "@tauri-apps/api/core";
import { getErrorMessage } from "@/lib/helpers";
import { toast } from "sonner";
import SettingsItem from "@/components/settings-item";
import { useTranslation } from "react-i18next";
import { useLocale } from "@/i18n/locale";
import { IClamAvVersion } from "@/lib/types";
import { getClamAvVersion } from "@/data/app";

export default function UpdateSettings(){
     const [updateState, setUpdateState] = useState<IDefsUpdaterState>(INITIAL_DEF_UPDATE_STATE);
     const [isChecked, setIsChecked] = useState(false);
     const [clamAvVersion, setClamavVersion] = useState<IClamAvVersion|null>(()=>JSON.parse(localStorage.getItem("clamav-version") as string) || null);
     const {t} = useTranslation("update");
     const {dateFns} = useLocale();
     const setState = useCallback((overrides: Partial<IDefsUpdaterState>) => setUpdateState(prev=>({ ...prev, ...overrides })),[])
     const handleUpdateDefs = useCallback(async()=>{
          if (updateState.isUpdatingDefs) return;
          try {
               await invoke("update_definitions")
          } catch (err) {
               toast.error(messageTxt("def-update-error"),{ description: getErrorMessage(err) })
          }
     },[updateState.isUpdatingDefs])
     const updateVersions = (parsed: Awaited<ReturnType<typeof getClamAvVersion>>) => {
          if(!parsed) return;
          const version: IClamAvVersion = {
               engine: parsed.engine,
               dbVersion: parsed.dbVersion
          }
          localStorage.setItem("clamav-version", JSON.stringify(version));
          setClamavVersion(version);
     }
     useEffect(()=>{
          (async()=>{
               try{
                    if(await invoke<boolean>("is_updating_definitions")) setState({isUpdatingDefs: true});
                    const clamAvVersion = await getClamAvVersion();
                    if(!clamAvVersion) return;
                    const stored = localStorage.getItem("last-updated");
                    // Background updates don't touch localStorage, so the database date can be newer.
                    const dates = [stored ? new Date(stored) : null, clamAvVersion.dbDate].filter(d => d && !isNaN(d.getTime())) as Date[];
                    setState({
                         lastUpdated: dates.length ? new Date(Math.max(...dates.map(d => d.getTime()))) : null,
                         isRequired: clamAvVersion.isOutdated,
                    });
                    updateVersions(clamAvVersion);
               } catch {
                    setState({
                         isRequired: true,
                    });
               } finally {
                    // Without a version (fresh install, no database) the tab must say "outdated", not spin forever.
                    setIsChecked(true);
               }
          })()
     },[])
     const {t: messageTxt} = useTranslation("messages")
     useEffect(()=>{
          const unsubs: Promise<UnlistenFn>[] = [
               listen("freshclam:start",() => setState({
                    isUpdatingDefs: true
               })),
               listen("freshclam:error",()=>
                    setUpdateState(prev=>({
                         ...prev,
                         isRequired: true,
                         isUpdatingDefs: false
                    }))
               ),
               listen<number>("freshclam:done",async(e)=>{
                    try{
                         // freshclam: 0 = updated, 1 = already up to date, anything else = failure
                         if(e.payload !== 0 && e.payload !== 1){
                              setState({
                                   isRequired: true,
                                   exitMsg: e.payload,
                                   isUpdatingDefs: false,
                              });
                              return;
                         }
                         const now = new Date();
                         localStorage.setItem("last-updated", now.toISOString());
                         setState({
                              isRequired: false,
                              lastUpdated: now,
                              exitMsg: e.payload,
                              isUpdatingDefs: false,
                         });
                         updateVersions(await getClamAvVersion());
                    } catch (err) {
                         toast.error(messageTxt("def-update-error"),{
                              description: getErrorMessage(err)
                         })
                    }
               })
          ];
          return () => {
               Promise.all(unsubs).then(fns=>fns.forEach(fn=>fn()));
          }
     },[])
     const {isRequired, exitMsg, lastUpdated, isUpdatingDefs} = updateState
     const isInitializing = !isChecked && !isUpdatingDefs
     const Icon = (isUpdatingDefs || isInitializing) ? Spinner : !isRequired ? CheckCircle : AlertCircle;
     const exitCodes = t("definitions.exit-codes",{returnObjects: true});
     const definitionsText = useMemo(()=>isUpdatingDefs ? "updating" : isInitializing ? "checking" : isRequired ? "outdated" : "updated",[isUpdatingDefs,isInitializing,isRequired])
     return (
          <div className="px-1 py-2 space-y-3">
               <SettingsItem
                    Icon={BugOff}
                    title={t("definitions.title")}
                    className="flex flex-col items-center gap-4"
               >
                    <div className="flex justify-center items-center gap-4">
                         <Icon className={cn("size-12",isRequired ? "text-destructive" : "text-emerald-600", (isUpdatingDefs || isInitializing) && "text-muted-foreground")}/>
                         <div className="text-center sm:text-left space-y-0.5">
                              <h2 className={cn(
                                   "text-xl md:text-2xl lg:text-3xl xl:text-[32px] font-semibold",
                                   isRequired && !isUpdatingDefs ? "text-red-900 dark:text-red-300" :
                                   (isUpdatingDefs || isInitializing) ? "text-muted-foreground" :
                                   "text-emerald-900 dark:text-emerald-300")}
                              >{t(`definitions.status.${definitionsText}`)}</h2>
                              {lastUpdated && (
                                   <p className="text-sm text-muted-foreground">
                                        {t("definitions.last-updated",{
                                             date: formatDistanceToNow(lastUpdated,{
                                                  includeSeconds: true,
                                                  addSuffix: true,
                                                  locale: dateFns
                                             })
                                        })}
                                   </p>
                              )}
                         </div>
                    </div>
                    <Button disabled={isUpdatingDefs} onClick={handleUpdateDefs}>
                         <RotateCw className={cn(isUpdatingDefs && "animate-spin")}/>
                         {isUpdatingDefs ? t("definitions.update.pending") : t("definitions.update.original")}
                    </Button>
                    {clamAvVersion && (
                         <p className="text-sm text-muted-foreground" title={t("definitions.version")}>{t("definitions.clamav-version",{
                              engine: clamAvVersion.engine,
                              dbVersion: clamAvVersion.dbVersion
                         })}</p>
                    )}
                    {exitMsg!==null && (
                         <p className="text-sm text-muted-foreground">{exitMsg <= 1 ? exitCodes[exitMsg] : t("definitions.update-failed",{code: exitMsg})}</p>
                    )}
               </SettingsItem>
          </div>
     )
}