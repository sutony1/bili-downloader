import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('worker', Path(__file__).with_name('transcribe_audio.py'))
TEST_ROOT = Path(__file__).resolve().parent / 'test-temp'
TEST_ROOT.mkdir(exist_ok=True)
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class TranscriptSafetyTests(unittest.TestCase):
    def test_unicode_source_and_existing_txt_are_preserved(self):
        with tempfile.TemporaryDirectory(dir=TEST_ROOT) as directory:
            assert Path(directory).resolve().is_relative_to(TEST_ROOT)
            source = Path(directory) / '手机录音 [测试].m4a'
            source.write_bytes(b'original audio')
            first = worker.write_result(source, ['中文文本', 'second paragraph'])
            original = first.read_bytes()
            second = worker.write_result(source, ['第二次'])
            self.assertNotEqual(first, second)
            self.assertEqual(first.read_bytes(), original)
            self.assertEqual(source.read_bytes(), b'original audio')
            self.assertEqual(first.read_text(encoding='utf-8-sig'), '中文文本\n\nsecond paragraph\n')
            self.assertFalse(list(Path(directory).glob('*.tmp')))

    def test_missing_audio_fails_before_loading_gpu(self):
        with tempfile.TemporaryDirectory(dir=TEST_ROOT) as directory:
            assert Path(directory).resolve().is_relative_to(TEST_ROOT)
            with self.assertRaisesRegex(RuntimeError, '不存在'):
                worker.transcribe(Path(directory) / 'missing.m4a', 'zh')

    def test_windows_simplified_chinese(self):
        if worker.os.name == 'nt':
            self.assertEqual(worker.simplify('語音轉寫測試'), '语音转写测试')


class ComputeTypeTests(unittest.TestCase):
    def test_modern_gpu_prefers_fp16(self):
        self.assertEqual(worker.select_compute_type(
            {'float32', 'float16', 'int8_float32', 'int8_float16'}), 'int8_float16')

    def test_gtx1060_uses_int8_float32(self):
        self.assertEqual(worker.select_compute_type(
            {'float32', 'int8', 'int8_float32'}), 'int8_float32')

    def test_unsupported_gpu_is_rejected(self):
        for supported in (set(), {'float32'}):
            with self.subTest(supported=supported):
                with self.assertRaisesRegex(RuntimeError, '不会自动改用 CPU'):
                    worker.select_compute_type(supported)


if __name__ == '__main__':
    unittest.main()
