import { load, type Store } from "@tauri-apps/plugin-store";
import { create } from "zustand";

const stores=new Map<string,Promise<Store>>();
const pending=new Map<string,{file:string;key:string;value:unknown;version:number}>();
let sequence=0;
let writes:Promise<void>=Promise.resolve();
export const useSettingsSaveState=create<{saving:boolean;error:string | null;changed:boolean}>(()=>({saving:false,error:null,changed:false}));

export function getSettingsStore(file:string):Promise<Store> {
  let store=stores.get(file);
  if(!store){store=load(file,{autoSave:true,defaults:{}});stores.set(file,store);store.catch(()=>stores.delete(file));}
  return store;
}
export function writeSetting(file:string,key:string,value:unknown):Promise<void> {
  const entry={file,key,value,version:++sequence};
  const id=`${file}:${key}`;pending.set(id,entry);
  useSettingsSaveState.setState({saving:true,changed:true});
  const write=writes.catch(()=>{}).then(async()=>{
    try{
      const store=await getSettingsStore(file);await store.set(key,value);
      if(typeof store.save==="function")await store.save();
      if(pending.get(id)?.version===entry.version)pending.delete(id);
      useSettingsSaveState.setState({saving:pending.size>0,error:null});
    }catch(error){useSettingsSaveState.setState({saving:false,error:`Không lưu được thiết lập: ${String(error)}`});throw error;}
  });
  writes=write;return write;
}
export async function flushSettings():Promise<void> {
  await writes.catch(()=>{});
  for(const entry of [...pending.values()])await writeSetting(entry.file,entry.key,entry.value);
  for(const store of stores.values()){const loaded=await store;if(typeof loaded.save==="function")await loaded.save();}
  useSettingsSaveState.setState({saving:false,error:null});
}
