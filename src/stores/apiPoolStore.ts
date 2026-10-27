import { create } from "zustand";
import { getApiPool, poolProvider, type ApiPoolSnapshot } from "../lib/apiPool";

interface ApiPoolState {
  snapshot: ApiPoolSnapshot;
  loaded: boolean;
  revision: number;
  error: string | null;
  dialogOpen: boolean;
  requestedProvider: string | null;
  missingProvider: string | null;
  refresh: () => Promise<void>;
  open: (provider?: string) => void;
  close: () => void;
  askForKey: (provider: string) => void;
  dismissMissing: () => void;
}
let requestSequence = 0;
export const useApiPoolStore = create<ApiPoolState>((set) => ({
  snapshot: { entries:[], defaults:{}, bindings:{} }, loaded:false, revision:0, error:null,
  dialogOpen:false, requestedProvider:null, missingProvider:null,
  refresh: async () => {
    const request = ++requestSequence;
    try { const snapshot = await getApiPool(); if (request === requestSequence) set({snapshot,loaded:true,error:null}); }
    catch (error) { if (request === requestSequence) set({loaded:true,error:String(error)}); }
  },
  open: (provider) => set({dialogOpen:true,requestedProvider:provider ? poolProvider(provider) : null,missingProvider:null}),
  close: () => set({dialogOpen:false}),
  askForKey: (provider) => set({missingProvider:poolProvider(provider)}),
  dismissMissing: () => set({missingProvider:null}),
}));
