import { act,cleanup,fireEvent,render,screen,waitFor } from "@testing-library/react";
import { afterEach,beforeEach,describe,expect,it,vi } from "vitest";
import type {ApiPoolSnapshot} from "../lib/apiPool";
const mock=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock("@tauri-apps/api/core",()=>({invoke:mock.invoke}));
import {ApiCredentialPicker} from "./ApiCredentialPicker";
import {ApiPoolDialogs} from "./ApiPoolDialogs";
import {useApiPoolStore} from "../stores/apiPoolStore";
import {selectedCredential} from "../lib/apiPool";
const snapshot=():ApiPoolSnapshot=>({entries:[{id:"personal",provider:"groq",label:"Cá nhân",region:null,key_present:true},{id:"work",provider:"groq",label:"Công việc",region:null,key_present:true}],defaults:{groq:"personal"},bindings:{}});
beforeEach(()=>{
  useApiPoolStore.setState({snapshot:snapshot(),loaded:true,error:null,dialogOpen:false,missingProvider:null,requestedProvider:null});
  mock.invoke.mockReset().mockImplementation(async(command:string)=>command==="get_api_pool" ? useApiPoolStore.getState().snapshot : null);
});
afterEach(cleanup);
describe("API Pool across AI roles",()=>{
  it("shares a default key and changes only the chosen role without reading a secret",async()=>{
    render(<><ApiCredentialPicker usage="copilot" provider="groq"/><ApiCredentialPicker usage="translation" provider="groq"/><ApiCredentialPicker usage="stt" provider="groq_whisper"/></>);
    expect(screen.getAllByText("Đã lưu · Cá nhân")).toHaveLength(3);
    mock.invoke.mockImplementation(async(command:string,args?:{usage:string;id:string})=>{
      if(command==="bind_api_pool_entry")useApiPoolStore.setState({snapshot:{...snapshot(),bindings:{[`${args!.usage}.groq`]:args!.id}}});
      if(command==="get_api_pool")return useApiPoolStore.getState().snapshot;
      return null;
    });
    fireEvent.change(screen.getByRole("combobox",{name:"API Pool key translation"}),{target:{value:"work"}});
    await waitFor(()=>expect(screen.getByText("Đã lưu · Công việc")).toBeInTheDocument());
    expect(screen.getAllByText("Đã lưu · Cá nhân")).toHaveLength(2);
    expect(mock.invoke).toHaveBeenCalledWith("bind_api_pool_entry",{usage:"translation",provider:"groq",id:"work"});
    expect(mock.invoke.mock.calls.some(([command])=>command==="get_api_key")).toBe(false);
  });
  it("opens the pool for a missing provider and Escape closes only that dialog",async()=>{
    render(<ApiPoolDialogs/>);
    act(()=>useApiPoolStore.getState().askForKey("gemini"));
    expect(screen.getByRole("dialog",{name:"Thiếu API key"})).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Mở API Pool"}));
    expect(screen.getByRole("dialog",{name:"API Pool"})).toBeInTheDocument();
    expect(screen.getByRole("combobox",{name:"API Pool provider"})).toHaveValue("gemini");
    fireEvent.keyDown(window,{key:"Escape"});
    await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });
  it("does not fall back to a different account when the selected key was deleted",()=>{
    const pool=snapshot();pool.bindings["translation.groq"]="";
    expect(selectedCredential(pool,"translation","groq")).toBeUndefined();
    expect(selectedCredential(pool,"copilot","groq")?.id).toBe("personal");
  });
});
