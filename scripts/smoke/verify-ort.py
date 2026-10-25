"""Check the optional packaged ONNX Runtime without loading a speech model."""
import ctypes
import json
import os
from pathlib import Path

directory = Path(__file__).resolve().parents[2] / 'out/windows'
os.environ['PATH'] = os.path.join(os.environ['SystemRoot'], 'System32') + os.pathsep + os.environ['SystemRoot']
with os.add_dll_directory(str(directory)):
    runtime = ctypes.CDLL(str(directory / 'onnxruntime.dll'))
    version_fn = ctypes.WINFUNCTYPE(ctypes.c_char_p)

    class ApiBase(ctypes.Structure):
        _fields_ = [('get_api', ctypes.c_void_p), ('get_version_string', version_fn)]

    runtime.OrtGetApiBase.restype = ctypes.POINTER(ApiBase)
    version = runtime.OrtGetApiBase().contents.get_version_string().decode('ascii')
    assert version == '1.24.2', version
    print(json.dumps({'loaded': True, 'version': version, 'path': str(directory / 'onnxruntime.dll')}))
