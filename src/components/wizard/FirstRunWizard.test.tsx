import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach,beforeEach,describe,expect,it,vi } from "vitest";
const mock=vi.hoisted(()=>({complete:vi.fn(),flush:vi.fn(),view:vi.fn(),settings:vi.fn(),toast:vi.fn()}));
vi.mock("../../stores/toastStore",()=>({showToast:mock.toast}));
vi.mock("../../stores/configStore",()=>({useConfigStore:(select:(s:unknown)=>unknown)=>select({setFirstRunCompleted:mock.complete})}));
vi.mock("../../stores/meetingStore",()=>({useMeetingStore:{getState:()=>({previousView:null,setSettingsOpen:mock.settings,setCurrentView:mock.view})}}));
vi.mock("../../lib/persistedSettings",()=>({flushSettings:mock.flush}));
vi.mock("../SettingsSaveStatus",()=>({SettingsSaveStatus:()=>null}));
vi.mock("../../settings/MeetingAudioSettings",()=>({MeetingAudioSettings:()=> <div>Audio setup</div>}));
vi.mock("../../settings/LocalModelsSettings",()=>({LocalModelsSettings:()=> <div>Local setup</div>}));
vi.mock("../../settings/LLMSettings",()=>({LLMSettings:()=> <div>Copilot setup</div>}));
vi.mock("../../settings/TranslationSettings",()=>({TranslationSettings:()=> <div>Translation setup</div>}));
import {FirstRunWizard} from "./FirstRunWizard";
beforeEach(()=>{vi.clearAllMocks();mock.flush.mockResolvedValue(undefined);});
afterEach(cleanup);
describe("quick setup exit",()=>{
  it.each([0,1,2,3,4])("can close step %i and leaves the wizard after saving",async(step)=>{
    render(<FirstRunWizard/>);
    for(let n=0;n<step;n++)fireEvent.click(screen.getByRole("button",{name:"Tiếp tục"}));
    fireEvent.click(screen.getByRole("button",{name:"Đóng thiết lập nhanh"}));
    await waitFor(()=>expect(mock.view).toHaveBeenCalledWith("launcher"));
    expect(mock.complete).toHaveBeenCalledWith(true);
    expect(mock.flush).toHaveBeenCalled();
    expect(mock.settings).toHaveBeenCalledWith(false);
  });
  it("Escape exits after a manual reopening as well",async()=>{
    render(<FirstRunWizard/>);
    fireEvent.keyDown(window,{key:"Escape"});
    await waitFor(()=>expect(mock.view).toHaveBeenCalledWith("launcher"));
  });
  it("reports a disk error and still allows the user to leave",async()=>{
    mock.flush.mockRejectedValueOnce(new Error("disk unavailable"));
    render(<FirstRunWizard/>);
    fireEvent.click(screen.getByRole("button",{name:"Thiết lập sau"}));
    await waitFor(()=>expect(mock.view).toHaveBeenCalledWith("launcher"));
    expect(mock.toast).toHaveBeenCalledWith(expect.stringContaining("Chưa lưu"),"error");
  });
});
