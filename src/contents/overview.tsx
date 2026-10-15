import { QUICK_ACCESS_LINKS } from "@/lib/constants/links";
import { lazy, Suspense, useEffect, useState } from "react";
import { useSettings } from "@/context/settings";
import { DefinitionStatus, Indicator } from "@/lib/types/enums";
import { checkDefinitionStatus } from "@/data/app";
import { SafetyIndicatorLoader } from "@/loaders/overview";
import OverviewCard from "@/components/cards/overview";
import { useTranslation } from "react-i18next";

const SafetyIndicator = lazy(()=>import("@/components/antivirus/indicator"));

export default function OverviewContent(){
     const {settings} = useSettings();
     const {t} = useTranslation("overview");
     const [definitionStatus, setDefinitionStatus] = useState<DefinitionStatus>(DefinitionStatus.Loading)
     const fetchDefinitionStatus = async()=>{
          try{
               const isOutdated = await checkDefinitionStatus()
               setDefinitionStatus(isOutdated ? DefinitionStatus.Outdated : DefinitionStatus.Updated)
          } catch {
               setDefinitionStatus(DefinitionStatus.Outdated)
          }
     }
     useEffect(()=>{
          fetchDefinitionStatus()
     },[])
     return (
          <>
               <Suspense fallback={<SafetyIndicatorLoader/>}>
                    <SafetyIndicator
                         definitionStatus={definitionStatus}
                         type={settings.realTime ? Indicator.Safe : Indicator.Alert}
                    />
               </Suspense>
               <h2 className="px-2 pb-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t("quick-actions")}</h2>
               <div className="grid grid-cols-2 md:grid-cols-4 w-full px-2 pb-4 gap-3">
                    {QUICK_ACCESS_LINKS.map((data,i)=>(
                         <OverviewCard
                              key={`item-${i+1}`}
                              data={data}
                         />
                    ))}
               </div>
          </>
     )
}