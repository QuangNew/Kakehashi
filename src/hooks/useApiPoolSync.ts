import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useApiPoolStore } from "../stores/apiPoolStore";

export function useApiPoolSync() {
  useEffect(()=>{
    let disposed=false;
    const subscription=listen("api_pool_changed",()=>{
      if(!disposed){void useApiPoolStore.getState().refresh();useApiPoolStore.setState(s=>({revision:s.revision+1}));}
    });
    void useApiPoolStore.getState().refresh();
    return()=>{disposed=true;void subscription.then(stop=>stop()).catch(()=>{});};
  },[]);
}
