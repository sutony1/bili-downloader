"""Local-only Faster-Whisper worker. JSON-lines IPC; never falls back to CPU."""
import argparse
import ctypes
import importlib.util
import json
import os
from pathlib import Path
import site
import shutil
import subprocess
import sys
import time

DLL_HANDLES = []
MODEL_ID = 'Systran/faster-whisper-large-v3'


def emit(**event):
    print(json.dumps(event, ensure_ascii=False), flush=True)


def dll_setup():
    if os.name != 'nt':
        return
    # NVIDIA pip runtimes and PyTorch's local DLLs, not arbitrary downloaded DLLs.
    roots = [Path(p) for p in site.getsitepackages()]
    roots.append(Path(site.getusersitepackages()))
    paths = []
    for root in roots:
        paths.extend(root / 'nvidia' / name / 'bin' for name in ('cublas', 'cudnn', 'cuda_runtime'))
        paths.append(root / 'torch' / 'lib')
    for path in paths:
        if path.is_dir():
            DLL_HANDLES.append(os.add_dll_directory(str(path)))
            os.environ['PATH'] = str(path) + os.pathsep + os.environ.get('PATH', '')


def model_dir():
    candidates = [Path(__file__).resolve().parent / 'models' / 'faster-whisper-large-v3',
                  Path.home() / 'Models' / 'whisper' / 'faster-whisper-large-v3']
    if os.environ.get('BILI_WHISPER_MODEL'):
        candidates.insert(0, Path(os.environ['BILI_WHISPER_MODEL']))
    for folder in candidates:
        if all((folder / name).is_file() for name in ('model.bin', 'config.json', 'tokenizer.json')):
            return str(folder)
    from huggingface_hub import snapshot_download
    try:
        return snapshot_download(MODEL_ID, local_files_only=True)
    except Exception:
        raise RuntimeError('未找到 large-v3 模型。请先运行“准备语音转写环境.cmd”，模型只需下载一次。') from None


def check():
    dll_setup()
    import ctranslate2
    if ctranslate2.get_cuda_device_count() < 1:
        raise RuntimeError('未检测到可用 NVIDIA GPU；本功能不会自动改用 CPU。')
    if 'int8_float16' not in ctranslate2.get_supported_compute_types('cuda'):
        raise RuntimeError('当前 GPU 不支持所需的 int8_float16 计算，请检查 NVIDIA 驱动。')
    folder = model_dir()
    return folder


def simplify(text):
    if os.name != 'nt' or not text:
        return text
    # Windows built-in conversion; avoid adding a separate Chinese-text dependency.
    flags = 0x02000000
    fn = ctypes.windll.kernel32.LCMapStringEx
    fn.argtypes = [ctypes.c_wchar_p, ctypes.c_uint, ctypes.c_wchar_p, ctypes.c_int, ctypes.c_wchar_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_ssize_t]
    fn.restype = ctypes.c_int
    size = fn('zh-CN', flags, text, len(text), None, 0, None, None, 0)
    if size <= 0:
        return text
    out = ctypes.create_unicode_buffer(size)
    return out.value if fn('zh-CN', flags, text, len(text), out, size, None, None, 0) else text


def write_result(source, paragraphs):
    text = '\n\n'.join(paragraphs).strip() + '\n'
    # Write a complete temporary file, then rename without replacing existing TXT.
    temporary = source.with_name(f'.{source.stem}.transcribe-{os.getpid()}.tmp')
    try:
        temporary.write_text(text, encoding='utf-8-sig')
        for number in range(10000):
            suffix = '' if number == 0 else f'_{number}'
            output = source.with_name(source.stem + '_转写' + suffix + '.txt')
            if output.exists():
                continue
            if os.name == 'nt':
                try:
                    temporary.rename(output)
                    return output
                except FileExistsError:
                    continue
            else:
                try:
                    os.link(temporary, output)
                    return output
                except FileExistsError:
                    continue
        raise RuntimeError('同名 TXT 文件太多，请更换保存位置。')
    finally:
        temporary.unlink(missing_ok=True)


def transcribe(source, language, report=emit):
    source = Path(source).expanduser().resolve()
    if not source.is_file():
        raise RuntimeError('录音文件不存在，请重新打开文件。')
    started = time.monotonic()
    report(status='正在检查 GPU 和 large-v3 模型…', busy=True, progress=0)
    folder = check()
    from faster_whisper import WhisperModel
    report(status='正在加载 large-v3 到 GPU…', busy=True)
    model = WhisperModel(folder, device='cuda', device_index=0,
                         compute_type='int8_float16', local_files_only=True, num_workers=1)
    report(status='正在识别录音…', busy=True, info='Faster-Whisper large-v3 · 本机 GPU · INT8/FP16')
    import numpy as np
    ffmpeg = os.environ.get('BILI_WHISPER_FFMPEG') or shutil.which('ffmpeg')
    if not ffmpeg:
        raise RuntimeError('未找到 FFmpeg，请将 ffmpeg.exe 放在下载助手旁或加入 PATH。')
    report(status='正在读取录音文件…', busy=True)
    decoded = subprocess.run([ffmpeg, '-v', 'error', '-nostdin', '-i', str(source), '-vn',
                              '-ac', '1', '-ar', '16000', '-f', 'f32le', 'pipe:1'],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             creationflags=0x08000000 if os.name == 'nt' else 0)
    if decoded.returncode:
        raise RuntimeError('无法解码录音：' + decoded.stderr.decode('utf-8', errors='replace')[-1000:])
    audio = np.frombuffer(decoded.stdout, dtype=np.float32).copy()
    if not audio.size:
        raise RuntimeError('文件中没有可读取的音轨。')
    segments, info = model.transcribe(audio, language=None if language == 'auto' else language,
                                      beam_size=5, vad_filter=True,
                                      condition_on_previous_text=False,
                                      vad_parameters={'min_silence_duration_ms': 500})
    paragraphs, paragraph, count = [], '', 0
    for segment in segments:
        text = segment.text.strip()
        if language == 'zh' or info.language == 'zh':
            text = simplify(text)
        if not text:
            continue
        joiner = '\n'  # preserve utterance boundaries without inventing punctuation
        paragraph += (joiner if paragraph else '') + text
        if len(paragraph) >= 180:
            paragraphs.append(paragraph)
            paragraph = ''
        count += 1
        percent = min(99, round(100 * segment.end / max(info.duration, .1)))
        report(status=f'正在转写 {percent}% · {segment.end:.0f} / {info.duration:.0f} 秒',
               busy=True, progress=percent, preview=text,
               info=f'语言：{info.language} · 已识别 {count} 段 · 本机 GPU')
    if paragraph:
        paragraphs.append(paragraph)
    if not paragraphs:
        raise RuntimeError('没有识别到有效语音，未生成空白 TXT。请检查录音或选择正确语言。')
    output = write_result(source, paragraphs)
    report(status='转写完成 ✓', busy=False, progress=100, output=str(output),
           info=f'录音 {info.duration:.1f} 秒 · 用时 {time.monotonic()-started:.1f} 秒 · 共 {count} 段',
           log=f'TXT 已保存到：{output}\nFaster-Whisper large-v3 · CUDA · int8_float16；未上传录音。')
    return output


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--input')
    parser.add_argument('--language', choices=['auto', 'zh', 'en'], default='auto')
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--prepare', action='store_true')
    args = parser.parse_args()
    if args.prepare:
        from huggingface_hub import snapshot_download
        folder = Path.home() / 'Models' / 'whisper' / 'faster-whisper-large-v3'
        print('Downloading official large-v3 model (~3 GB). Audio is never uploaded.', flush=True)
        snapshot_download(MODEL_ID, local_dir=str(folder),
                          allow_patterns=['model.bin', 'config.json', 'tokenizer.json', 'vocabulary.*', 'preprocessor_config.json'])
        print('Model ready:', folder)
    elif args.check:
        folder = check()
        emit(status='GPU 和 large-v3 模型检查通过', busy=False, info='本机 NVIDIA GPU · large-v3 · INT8/FP16', log=folder)
    elif args.input:
        transcribe(args.input, args.language)
    else:
        parser.error('--input or --check is required')


if __name__ == '__main__':
    sys.stdout.reconfigure(encoding='utf-8')
    try:
        main()
    except Exception as error:
        message = str(error)
        if any(s in message.lower() for s in ['out of memory', 'cuda_error_out_of_memory']):
            message = 'GPU 显存不足。请关闭其他占用显存的程序后重试；未降级到 CPU。'
        emit(status='转写失败', busy=False, error=True, log=message)
        sys.exit(1)
