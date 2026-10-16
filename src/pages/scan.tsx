import ScanContent from "@/contents/scan";
import { ScanType } from "@/lib/types/enums";
import { useParams, useSearchParams } from "react-router";

export default function ScanPage(){
     const [searchParams] = useSearchParams();
     const path = searchParams.getAll("path");
     const {type} = useParams<{type: ScanType}>();
     return <ScanContent key={`${type}?${searchParams.toString()}`} type={type} path={path}/>
}
