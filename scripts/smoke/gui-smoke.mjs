import { mkdir, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

await mkdir(new URL('../../.cache/smoke/', import.meta.url), {recursive:true});
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
async function connect(target) {
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let nextId = 0;
  const pending = new Map();
  const errors = [];
  socket.onmessage = event => {
    const message = JSON.parse(event.data);
    if (message.method === 'Runtime.exceptionThrown') errors.push(message.params.exceptionDetails);
    if (!message.id) return;
    const task = pending.get(message.id);
    if (!task) return;
    pending.delete(message.id);
    clearTimeout(task.timer);
    if (message.error) task.reject(new Error(JSON.stringify(message.error))); else task.resolve(message.result);
  };
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++nextId;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timed out: ${method}`)); }, 130000);
    pending.set(id, { resolve, reject, timer });
    socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async expression => {
    const result = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
    return result.result?.value;
  };
  await send('Runtime.enable');
  return { socket, evaluate, errors, send };
}

let client;
const connections = [];
try {
  const targets = await (await fetch('http://127.0.0.1:9227/json')).json();
  for (const target of targets.filter(target => target.type === 'page')) {
    const connection = await connect(target);
    connections.push(connection);
    const label = await connection.evaluate('window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label');
    if (label === 'launcher') { client = connection; break; }
  }
  assert(client, 'Launcher webview was not found');
  const evaluate = client.evaluate;
  if (process.argv.includes('--quit')) {
    await evaluate('setTimeout(() => window.__TAURI_INTERNALS__.invoke("plugin:process|exit", {code: 0}), 250); true');
    console.log('Requested normal desktop exit');
    for (const connection of connections) connection.socket.close();
    process.exit(0);
  }
  if (process.argv.includes('--verify-realtime') || process.argv.includes('--verify-restored-scope')) {
    const restoredScope = process.argv.includes('--verify-restored-scope');
    // Inspect the packaged UI without changing settings, creating a meeting,
    // starting transcription/recording or calling a cloud generation API.
    await waitFor('!!document.querySelector(\'button[aria-label="Settings (Ctrl+,)"]\')', 'launcher');
    assert.equal(await evaluate('document.title'), 'Kakehashi');
    const engines = JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("list_local_stt_engines")'));
    assert(engines.find(engine => engine.engine === 'whisper_cpp')?.models.some(model => model.definition.model_id === 'small' && model.is_downloaded), 'Whisper Small was not found');
    const nativeProviders = JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("get_llm_providers")'));
    const copilotProviders = nativeProviders.map(provider => provider.type);
    for (const provider of ['local_gguf', 'ollama', 'lm_studio', 'claude_cli', 'groq', 'gemini']) assert(copilotProviders.includes(provider), provider);
    const nativeStt = JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("get_available_stt_providers")'));
    const sttProviders = [...new Set([...nativeStt.map(provider => provider.type ?? provider.provider_type), ...engines.map(engine => engine.engine), 'web_speech'])];
    for (const provider of ['whisper_cpp', 'windows_native', 'deepgram', 'whisper_api', 'azure_speech', 'groq_whisper', 'sherpa_onnx', 'ort_streaming', 'parakeet_tdt']) assert(sttProviders.includes(provider), provider);
    await evaluate('document.querySelector(\'button[aria-label="Settings (Ctrl+,)"]\').click()');
    await click('STT Providers');
    await waitFor('document.body.innerText.includes("Chia đoạn phụ đề trực tiếp")', 'bounded audio segmentation settings');
    assert(await evaluate('document.body.innerText.includes("Web Speech")'), 'Web Speech settings missing');
    const segmentation = await evaluate(`Object.fromEntries(['Độ dài tối thiểu', 'Độ dài tối đa', 'Khoảng lặng chốt câu'].map(label => [label, Number(document.querySelector('input[aria-label="' + label + '"]').value)]))`);
    assert(segmentation['Độ dài tối thiểu'] <= segmentation['Độ dài tối đa']);
    assert(segmentation['Độ dài tối đa'] >= 1 && segmentation['Độ dài tối đa'] <= 10);
    assert(segmentation['Khoảng lặng chốt câu'] >= 0.2 && segmentation['Khoảng lặng chốt câu'] <= 2);
    await evaluate('document.querySelector(\'input[aria-label="Độ dài tối đa"]\').scrollIntoView({block: "center"})');
    const screenshot = await client.send('Page.captureScreenshot', {format: 'png', captureBeyondViewport: false});
    await writeFile(new URL(`../../.cache/smoke/${restoredScope ? 'restored-scope-settings' : 'realtime-settings'}.png`, import.meta.url), Buffer.from(screenshot.data, 'base64'));
    await click('Translation');
    await waitFor('!!document.querySelector(\'select[aria-label="Nhà cung cấp AI dịch"]\')', 'translation provider');
    const translationProviders = await evaluate('Array.from(document.querySelector(\'select[aria-label="Nhà cung cấp AI dịch"]\').options, option => option.value)');
    assert.deepEqual(translationProviders, ['hy-mt', 'groq', 'gemini', 'microsoft', 'google', 'deepl', 'opus-mt', 'llm']);
    await click('LLM Providers');
    await waitFor('document.body.innerText.includes("LM Studio") && document.body.innerText.includes("Ollama") && document.body.innerText.includes("Claude")', 'restored copilot choices');
    await click('Noise Presets');
    await waitFor('document.body.innerText.includes("Noise")', 'noise preferences');
    await click('General');
    await waitFor('!!document.querySelector(\'button[aria-label="Toggle start on login"]\')', 'start on login preference');
    await evaluate('document.querySelector(\'button[aria-label="Back to launcher"]\').click()');
    await click('Start Meeting');
    await waitFor('!!document.querySelector(\'[role="dialog"][aria-label="Meeting setup"]\')', 'meeting setup');
    await evaluate('(() => { const dialog = document.querySelector(\'[role="dialog"][aria-label="Meeting setup"]\'); const change = [...dialog.querySelectorAll("button")].find(button => button.textContent.includes("Change")); change?.click(); })()');
    assert(await evaluate('document.body.innerText.includes("In-Person")'), 'In-person mode missing');
    await click('Team Meeting');
    await waitFor('document.body.innerText.includes("Lecture") && document.body.innerText.includes("Webinar") && document.body.innerText.includes("Oral Exam")', 'restored scenarios');
    await evaluate('window.dispatchEvent(new KeyboardEvent("keydown", {key:"Escape"}))');
    await pause(500);
    assert.equal(client.errors.length, 0, JSON.stringify(client.errors));
    const report = {ok: true, app: 'Kakehashi', whisperSmallAvailable: true, segmentation, translationProviders, copilotProviders, sttProviders, restoredGroups: ['G4','G5','G6','G7','G9','G10'], uncaughtExceptions: client.errors.length, createdMeeting: false, startedTranscription: false, calledCloudApi: false};
    await writeFile(new URL(`../../.cache/smoke/${restoredScope ? 'restored-scope-smoke-result' : 'realtime-smoke-result'}.json`, import.meta.url), JSON.stringify(report, null, 2));
    console.log(JSON.stringify(report));
    for (const connection of connections) connection.socket.close();
    process.exit(0);
  }
  if (process.argv.includes('--verify-restart')) {
    await waitFor('document.body.innerText.includes("Kakehashi") && (document.body.innerText.includes("local_gguf") || document.body.innerText.includes("Local GGUF"))', 'restored launcher');
    const body = await evaluate('document.body.innerText');
    assert.equal(await evaluate('document.title'), 'Kakehashi');
    assert(body.includes('Kakehashi'), 'App display name was not changed');
    assert(!body.includes('Thiết lập Việt–Nhật bằng model trong thư mục'), 'First-run flag was not restored');
    assert(body.includes('local_gguf') || body.includes('Local GGUF'), 'Local copilot was not restored');
    const engines = JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("list_local_stt_engines")'));
    assert(engines.find(engine => engine.engine === 'whisper_cpp')?.models.some(model => model.definition.model_id === 'small' && model.is_downloaded), 'Whisper Small is not available after restart');
    const rag = JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("get_rag_config")'));
    assert.equal(rag.embedding_provider, 'local_gguf');
    assert.equal(rag.embedding_dimensions, 1024);
    const budget = JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("get_token_budget")'));
    const localConfig = await evaluate('window.__TAURI_INTERNALS__.invoke("get_local_models_config")');
    assert.equal(budget.limit, localConfig.copilot.context, 'Token budget ignored the local context');
    await waitFor(`document.querySelector('[role="meter"][aria-label="Token budget usage"]')?.getAttribute('aria-valuemax') === '${budget.limit}'`, 'displayed local context budget');
    const models = JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("list_models", {provider: "local_gguf"})'));
    assert(models.some(model => model.id === 'copilot'), JSON.stringify(models));
    const translation = await evaluate('window.__TAURI_INTERNALS__.invoke("translate_text", {text: "来月末までに完了できますか？", sourceLang: "ja", targetLang: "vi"})');
    assert(translation.translated_text?.trim());
    const existingResources = JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("list_context_resources")'));
    let fallback = {skipped: 'Existing context resources preserved'};
    if (existingResources.length === 0) {
      const fixturePath = fileURLToPath(new URL('./kakehashi-smoke-context.txt', import.meta.url));
      fallback = await evaluate(`(async () => {
        const invoke = window.__TAURI_INTERNALS__.invoke;
        const resource = JSON.parse(await invoke('load_context_file', {filePath: ${JSON.stringify(fixturePath)}}));
        const starts = [], tokens = [], listeners = [];
        try {
          for (const [event, receive] of [['llm_stream_start', value => starts.push(value)], ['llm_stream_token', value => tokens.push(value.token || '')]]) {
            const handler = window.__TAURI_INTERNALS__.transformCallback(event => receive(event.payload));
            const id = await invoke('plugin:event|listen', {event, target: {kind: 'Any'}, handler});
            listeners.push({event, id});
          }
          await invoke('generate_assist', {mode: 'AskQuestion', customQuestion: 'Theo tài liệu đính kèm, mã dự án thử nghiệm là gì? Trả lời ngắn bằng tiếng Việt.', transcriptSegments: '[]', requestId: 'kakehashi-upstream-smoke'});
          await new Promise(resolve => setTimeout(resolve, 200));
          return {sentFixtureToModel: starts.some(value => JSON.stringify(value).includes('HASHI-482')), replyHasFixtureCode: tokens.join('').includes('HASHI-482')};
        } finally {
          await invoke('remove_context_file', {resourceId: resource.id});
          for (const {event, id} of listeners) {
            window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(event, id);
            await invoke('plugin:event|unlisten', {event, eventId: id});
          }
        }
      })()`);
      assert(fallback.sentFixtureToModel, 'The unindexed attached document was not passed to the local copilot');
      assert(fallback.replyHasFixtureCode, 'The local copilot did not use the attached document');
      assert.equal(JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("list_context_resources")')).length, 0);
    }
    const scopeUi = await inspectScopeUi();
    assert.equal(client.errors.length, 0, JSON.stringify(client.errors));
    const screenshot = await client.send('Page.captureScreenshot', {format: 'png', captureBeyondViewport: false});
    await writeFile(new URL('../../.cache/smoke/launcher.png', import.meta.url), Buffer.from(screenshot.data, 'base64'));
    const report = {ok: true, app: 'Kakehashi', localContext: budget.limit, documentFallback: fallback, scopeUi, restoredCopilot: 'local_gguf', restoredWhisperSmall: true, restoredRagDimensions: rag.embedding_dimensions, uncaughtExceptions: client.errors.length, translation: translation.translated_text};
    await writeFile(new URL('../../.cache/smoke/restart-smoke-result.json', import.meta.url), JSON.stringify(report, null, 2));
    console.log(JSON.stringify(report));
    for (const connection of connections) connection.socket.close();
    process.exit(0);
  }
  async function waitFor(expression, label, timeout = 130000) {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      if (await evaluate(expression)) return;
      await pause(300);
    }
    throw new Error(`Timed out waiting for ${label}: ${await evaluate('document.body.innerText.slice(-3000)')}`);
  }
  async function click(text) {
    const expression = `(() => { const button = [...document.querySelectorAll('button')].find(button => button.textContent.includes(${JSON.stringify(text)}) && button.getBoundingClientRect().width > 0); if (!button || button.disabled) return false; button.click(); return true; })()`;
    await waitFor(expression, text);
  }
  async function inspectScopeUi() {
    const nativeProviders = JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("get_llm_providers")'));
    const nativeStt = JSON.parse(await evaluate('window.__TAURI_INTERNALS__.invoke("get_available_stt_providers")'));
    assert(nativeStt.some(p => (p.type ?? p.provider_type) === 'windows_native') && nativeStt.some(p => (p.type ?? p.provider_type) === 'groq_whisper'));
    assert(nativeProviders.some(p => p.type === 'groq') && nativeProviders.some(p => p.type === 'gemini'));
    for (const provider of ['ollama','lm_studio','claude_cli']) assert(nativeProviders.some(p => p.type === provider));
    await waitFor(`(() => { const button = document.querySelector('button[aria-label="Settings (Ctrl+,)"]'); if (!button) return false; button.click(); return true; })()`, 'open settings');
    await click('Translation');
    const translationSelect = 'select[aria-label="Nhà cung cấp AI dịch"]';
    await waitFor(`!!document.querySelector(${JSON.stringify(translationSelect)})`, 'translation provider choices');
    const translationProviders = await evaluate(`Array.from(document.querySelector(${JSON.stringify(translationSelect)}).options, o => o.value)`);
    assert.deepEqual(translationProviders,['hy-mt','groq','gemini','microsoft','google','deepl','opus-mt','llm']);
    await evaluate(`(() => { const select = document.querySelector(${JSON.stringify(translationSelect)}); select.value = 'gemini'; select.dispatchEvent(new Event('change',{bubbles:true})); })()`);
    await waitFor(`!!document.querySelector('input[aria-label="API key AI dịch"][type="password"]')`, 'dedicated translation API form');
    await pause(500);
    const shot = await client.send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
    await writeFile(new URL('../../.cache/smoke/translation-settings.png',import.meta.url),Buffer.from(shot.data,'base64'));
    await click('LLM Providers');
    await waitFor('document.body.innerText.includes("Ollama") && document.body.innerText.includes("LM Studio")', 'copilot provider choices');
    const copilotProviders = nativeProviders.map(provider => provider.type);
    assert(copilotProviders.includes('groq') && copilotProviders.includes('gemini') && copilotProviders.includes('local_gguf'));
    await click('Audio & Devices');
    await waitFor('document.querySelectorAll(\'select[aria-label$="recognition language"]\').length === 2', 'audio source languages');
    const languages = await evaluate('Array.from(document.querySelectorAll(\'select[aria-label$="recognition language"]\'), select => select.value.split("-")[0])');
    assert.deepEqual([...languages].sort(),['ja','vi']);
    await click('General');
    await waitFor(`document.querySelector('[data-testid="app-data-directory"]')?.textContent.includes('com.tvnjp.app')`, 'actual Tauri data directory');
    await evaluate(`(() => { const button = document.querySelector('button[aria-label="Back to launcher"]') || document.querySelector('button[aria-label="Close settings"]') || [...document.querySelectorAll('button')].find(b => b.textContent.includes('Back')); if (!button) throw new Error('Settings close button not found'); button.click(); })()`);
    return {translationProviders,copilotProviders,stt:nativeStt.map(provider => provider.type ?? provider.provider_type),languages,actualDataDirectory:true};
  }
  const modelsInput = JSON.stringify('input[aria-label="Models folder"]');
  const runtimeInput = JSON.stringify('input[aria-label="Runtime executable"]');
  if (!await evaluate(`!!document.querySelector(${modelsInput})`)) {
    if (await evaluate('document.body.innerText.includes("Chào mừng") && !document.querySelector(\'button[aria-label="Settings (Ctrl+,)"]\')')) {
      await click('Tiếp tục');
    } else {
      const settingsButton = JSON.stringify('button[aria-label="Settings (Ctrl+,)"]');
      await waitFor(`(() => { const button = document.querySelector(${settingsButton}); if (!button) return false; button.click(); return true; })()`, 'settings button');
      await click('Local Models · VI/JA');
    }
  }
  await waitFor(`!!document.querySelector(${modelsInput})`, 'model settings');
  const paths = await evaluate(`({ models: document.querySelector(${modelsInput}).value, runtime: document.querySelector(${runtimeInput}).value })`);
  assert(paths.models.endsWith('models'));
  await click('Áp dụng chế độ Việt–Nhật');
  await waitFor('document.body.innerText.includes("Đã bật phụ đề hai chiều")', 'bilingual preset');
  await click('Dùng copilot local');
  await waitFor('document.body.innerText.includes("Copilot local đã sẵn sàng")', 'copilot activation');
  await click('Dùng RAG local (1024d)');
  await waitFor('document.body.innerText.includes("RAG local đã bật")', 'local RAG activation');
  const status = await evaluate('window.__TAURI_INTERNALS__.invoke("get_local_models_status")');
  assert(status.every(role => role.state === 'running'), JSON.stringify(status));
  const screenshot = await client.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
  await writeFile(new URL('../../.cache/smoke/local-models.png', import.meta.url), Buffer.from(screenshot.data, 'base64'));
  for (let i=0;i<4;i++) {
    if (await evaluate('[...document.querySelectorAll("button")].some(button => button.textContent === "Tiếp tục")')) { await click('Tiếp tục'); await pause(200); }
  }
  if (await evaluate('[...document.querySelectorAll("button")].some(button => button.textContent === "Hoàn tất")')) await click('Hoàn tất');
  await pause(1500);
  assert.equal(await evaluate('document.body.innerText.includes("Thiết lập Việt–Nhật bằng model trong thư mục")'), false);
  assert.equal(client.errors.length, 0, JSON.stringify(client.errors));
  const report = { ok: true, paths, roles: status.map(({ role, state, device }) => ({ role, state, device })), configured: ['VI mic -> JA', 'JA system -> VI', 'local copilot', 'local 1024d RAG'], uncaughtExceptions: client.errors.length };
  await writeFile(new URL('../../.cache/smoke/gui-smoke-result.json', import.meta.url), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally {
  for (const connection of connections) connection.socket.close();
}
