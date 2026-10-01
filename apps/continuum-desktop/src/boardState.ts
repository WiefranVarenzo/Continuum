export interface BoardLink {id:string;source:string;target:string;label:string;reason?:string;origin:"user"|"ai"|"project"}
export interface BoardSnapshot {
  positions:Record<string,{x:number;y:number}>; links:BoardLink[]; hiddenEdges:string[]; hiddenNodes:string[];
  groups:Record<string,string>; notes:Record<string,{title:string;description:string}>;
  dismissedPairs?:string[];
}
export interface BoardHistory {present:BoardSnapshot;past:BoardSnapshot[];future:BoardSnapshot[];appliedJobs:string[]}
export const emptyBoard:BoardHistory={present:{positions:{},links:[],hiddenEdges:[],hiddenNodes:[],groups:{},notes:{}},past:[],future:[],appliedJobs:[]};
export function changeBoard(history:BoardHistory,next:BoardSnapshot):BoardHistory {
  if(JSON.stringify(history.present)===JSON.stringify(next))return history;
  return {...history,present:next,past:[...history.past,history.present].slice(-40),future:[]};
}
export function undoBoard(h:BoardHistory):BoardHistory {return h.past.length?{...h,present:h.past[h.past.length-1],past:h.past.slice(0,-1),future:[h.present,...h.future].slice(0,40)}:h;}
export function redoBoard(h:BoardHistory):BoardHistory {return h.future.length?{...h,present:h.future[0],past:[...h.past,h.present].slice(-40),future:h.future.slice(1)}:h;}
export function visibleLinks(h:BoardSnapshot,project:BoardLink[],ids:Set<string>) {
  const links=new Map(project.map(e=>[e.id,e]));for(const link of h.links)links.set(link.id,link);
  return [...links.values()].filter(e=>!h.hiddenEdges.includes(e.id)&&ids.has(e.source)&&ids.has(e.target));
}
