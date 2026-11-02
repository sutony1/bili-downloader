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
execute("window.updateTranscription({busy:false}); switchPlatform('bili'); window.update({busy:true})");
messages.length = 0;
execute('window.receiveDroppedFiles(droppedPaths)');
assert.equal(messages.length, 0, 'busy downloads must ignore drops');
console.log('PASS: native file drop bridge, Unicode paths, hover, tab and busy guards');
