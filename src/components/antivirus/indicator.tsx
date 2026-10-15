import { cn } from "@/lib/utils";
import { AlertCircle, CheckCircle, Search, SearchCheck, ShieldCheck } from "lucide-react"
import { Spinner } from "../ui/spinner";
import { useMemo } from "react";
import { formatDistanceToNow } from "date-fns";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { useLocale } from "@/i18n/locale";
import { DefinitionStatus, Indicator } from "@/lib/types/enums";
import { Button } from "../ui/button";
import { useSettings } from "@/context/settings";

interface Props{
     type: Indicator
     definitionStatus: DefinitionStatus
}
// Shield artwork per state: green = protected, red = protection off, blue = checking / needs attention.
export default function SafetyIndicator({type, definitionStatus}: Props){
     const lastScanned = useMemo(()=>{
          const stored = localStorage.getItem("last-scanned");
          return stored ? new Date(parseInt(stored)) : null;
     },[]);
     const {t} = useTranslation("overview");
     const {dateFns} = useLocale()
     const {setSettings} = useSettings()
     const navigate = useNavigate()
     const outdated = definitionStatus==="outdated"
     const shield = type==="alert" ? "red" : (outdated || definitionStatus==="loading" || type==="warning") ? "blue" : "green"
     return (
          <section className="relative flex flex-col items-center text-center gap-5 pt-6 pb-10">
               <div aria-hidden className={cn(
                    "absolute top-4 size-56 rounded-full blur-3xl opacity-30 -z-10",
                    shield==="green" && "bg-emerald-500",
                    shield==="red" && "bg-red-500",
                    shield==="blue" && "bg-cyan-500"
               )}/>
               <img src={`/shield-${shield}.webp`} alt="" className="size-36 sm:size-44 object-contain drop-shadow-xl"/>
               <div className="space-y-2">
                    <h1 className={cn(
                         "text-3xl sm:text-4xl font-semibold leading-tight",
                         shield==="red" && "text-red-600 dark:text-red-400",
                         outdated && "text-amber-600 dark:text-amber-400"
                    )}>
                         {t(`indicator.${outdated ? "outdated" : type}`)}
                    </h1>
                    <ul className="flex flex-wrap justify-center gap-x-5 gap-y-1 text-sm text-muted-foreground">
                         <li className="flex items-center gap-1.5">
                              <SearchCheck className="size-4"/>
                              {lastScanned ? (
                                   <span>
                                        {t("indicator.last-scan")}{" "}
                                        {formatDistanceToNow(lastScanned,{
                                             includeSeconds: true,
                                             addSuffix: true,
                                             locale: dateFns
                                        })}
                                   </span>
                              ) : t("indicator.never-scanned")}
                         </li>
                         <li className="flex items-center gap-1.5">
                              {definitionStatus==="loading" ? <Spinner className="size-4"/> : outdated ? <AlertCircle className="size-4"/> : <CheckCircle className="size-4"/>}
                              {t("definition.title",{
                                   status: t(`definition.${definitionStatus}`)
                              })}
                         </li>
                    </ul>
               </div>
               <div className="flex flex-wrap justify-center gap-2">
                    <Button size="lg" className="rounded-full px-7" onClick={()=>navigate("/scan/main")}>
                         <Search/>
                         {t("scan-now")}
                    </Button>
                    {type==="alert" && (
                         <Button size="lg" variant="outline" className="rounded-full px-6" onClick={()=>setSettings({realTime: true})}>
                              <ShieldCheck/>
                              {t("turn-on-realtime")}
                         </Button>
                    )}
               </div>
          </section>
     )
}
