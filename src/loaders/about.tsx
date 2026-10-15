import { Skeleton } from "@/components/ui/skeleton";

export function AboutContentLoader(){
     return (
          <div className="max-w-xl mx-auto py-8 flex flex-col items-center gap-8">
               <Skeleton className="h-20 w-72"/>
               <Skeleton className="h-36 w-full rounded-2xl"/>
               <Skeleton className="h-8 w-80"/>
          </div>
     )
}
