const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');

const elements = new Map();
function element(id) {
  if (!elements.has(id)) elements.set(id, {
    value: '', textContent: '', hidden: false, disabled: false, attributes: {},
    classes: new Set(), setAttribute(key, value) { this.attributes[key] = value; },
    replaceChildren() {}, append() {}, focus() {},
    classList: { toggle(key, enabled) { const classes = element(id).classes; enabled ? classes.add(key) : classes.delete(key); } }
  });
  return elements.get(id);
}
const messages = [];
const context = vm.createContext({
  document: { getElementById: element, createElement: () => ({} ) },
  window: { ipc: { postMessage: text => messages.push(JSON.parse(text)) }, addEventListener() {} }
});
const html = fs.readFileSync(path.join(__dirname, '../src/ui.html'), 'utf8');
vm.runInContext(html.match(/<script>([\s\S]*?)<\/script>/)[1], context);
const execute = source => vm.runInContext(source, context);
execute("window.receiveDroppedFiles(['C:/录音.m4a'])");
assert.equal(messages.length, 0, 'other tabs must ignore dropped audio');
execute("switchPlatform('transcribe')");
messages.length = 0;
execute('window.transcriptionDrag(true)');
assert(element('audio-drop-zone').classes.has('dragging'));
const input = 'C:/很深的目录/手机录音 [测试] #1.m4a';
context.droppedPaths = [input];
execute('window.receiveDroppedFiles(droppedPaths)');
assert.deepEqual(messages, [{action: 'transcribe-drop', platform: 'transcribe', paths: [input]}]);
assert(!element('audio-drop-zone').classes.has('dragging'));
assert(!messages.some(x => x.action === 'transcribe-start'), 'drop must not start transcription');
execute("window.updateTranscription({input:'C:/录音.m4a',busy:true})");
messages.length = 0;
execute('window.transcriptionDrag(true); window.receiveDroppedFiles(droppedPaths)');
assert.equal(messages.length, 0, 'busy transcription must keep its file');
assert(!element('audio-drop-zone').classes.has('dragging'));
assert.equal(element('audio-drop-zone').attributes['aria-disabled'], 'true');
assert.equal(element('audio-clear').disabled, true);
execute('clearTranscription()');
assert.equal(messages.length, 0, 'busy transcription must reject clearing');
execute("window.updateTranscription({busy:false,input:'C:/录音.m4a',output:'C:/录音_转写.txt',preview:'旧的转写内容',progress:100})");
execute('clearTranscription()');
assert.equal(messages.at(-1).action, 'transcribe-clear');
execute("window.updateTranscription({input:'',output:'',preview:'',progress:0,status:'请打开或拖入下一个录音文件',info:'large-v3 · 本机 GPU',log:'已清空当前选择和转写显示；已保存的录音及 TXT 文件均保留。'})");
assert.equal(element('audio-file').value, '');
assert.equal(element('audio-output').textContent, '');
assert.equal(element('audio-preview').textContent, '');
assert.equal(element('audio-preview').hidden, true);
assert.equal(element('audio-progress').value, 0);
assert.equal(element('audio-start').disabled, true);
assert.equal(element('audio-open').disabled, true);
assert.equal(element('audio-folder').disabled, true);
assert.equal(element('audio-clear').disabled, false);
execute("window.updateTranscription({busy:false}); switchPlatform('bili'); window.update({busy:true})");
messages.length = 0;
execute('window.receiveDroppedFiles(droppedPaths)');
assert.equal(messages.length, 0, 'busy downloads must ignore drops');
execute('clearTranscription()');
assert.equal(messages.length, 0, 'busy downloads must reject clearing');
console.log('PASS: native file drop bridge, Unicode paths, hover, tab and busy guards');
