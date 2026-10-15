import { Skeleton } from "@/components/ui/skeleton";

export function SafetyIndicatorLoader(){
     return (
          <div className="flex flex-col items-center gap-5 pt-6 pb-10">
               <Skeleton className="size-36 sm:size-44 rounded-full"/>
               <Skeleton className="h-9 w-72"/>
               <Skeleton className="h-4 w-80"/>
               <Skeleton className="h-10 w-44 rounded-full"/>
          </div>
     )
}
export default function OverviewLoader(){
     return (
          <>
               <SafetyIndicatorLoader/>
               <div className="grid grid-cols-2 md:grid-cols-4 w-full px-2 pb-4 gap-3">
                    {Array.from({length: 8}).map((_,i)=>(
                         <Skeleton key={`item-${i+1}`} className="w-full h-32 rounded-xl"/>
                    ))}
               </div>
          </>
     )
}
