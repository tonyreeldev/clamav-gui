import { cn } from "@/lib/utils";

interface Props{
     size?: "sm" | "md" | "lg",
     className?: string
}
const SIZES = {
     sm: { icon: "size-8", text: "text-base" },
     md: { icon: "size-12", text: "text-2xl" },
     lg: { icon: "size-20", text: "text-4xl" },
}
// The name is live text (not part of the image) so it follows the light/dark theme.
export default function Logo({size="sm", className}: Props){
     return (
          <div className={cn("flex items-center gap-2.5", className)}>
               <img src="/shield-blue.webp" alt="" className={cn("object-contain shrink-0", SIZES[size].icon)}/>
               <span className={cn("font-semibold tracking-tight whitespace-nowrap", SIZES[size].text)}>
                    ClamAV <span className="text-primary">GUI</span>
               </span>
          </div>
     )
}
