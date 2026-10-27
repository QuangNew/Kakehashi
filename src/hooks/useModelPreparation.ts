import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useConfigStore } from "../stores/configStore";
import { useMeetingStore } from "../stores/meetingStore";
import { usePreparationStore, type PreparationStatus } from "../stores/preparationStore";

export function useModelPreparation(){
  const loaded=useConfigStore(s=>s._loaded);
  const enabled=useConfigStore(s=>s.preloadLocalModels);
  const audio=useConfigStore(s=>s.meetingAudioConfig);
  const active=useMeetingStore(s=>s.activeMeeting != null);
  useEffect(()=>{
    let disposed=false;
    let eventReceived=false;
    const unlisten=listen<PreparationStatus>("model_preparation",event=>{eventReceived=true;if(!disposed)usePreparationStore.setState({status:event.payload});});
    void invoke<PreparationStatus>("get_model_preparation").then(status=>{if(!disposed && !eventReceived)usePreparationStore.setState({status});}).catch(()=>{});
    return ()=>{disposed=true;void unlisten.then(fn=>fn());};
  },[]);
  useEffect(()=>{
    if(!loaded || !enabled || !audio || active) return;
    const timer=setTimeout(()=>void usePreparationStore.getState().prepare(),1200);
    return ()=>clearTimeout(timer);
  },[loaded,enabled,audio,active]);
}
