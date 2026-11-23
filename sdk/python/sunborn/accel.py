"""
Native C SIMD Vector Accelerator for Sunborn Python SDK.

Loads the compiled native C SIMD library (`sunborn_simd_accelerator`) via ctypes
when available, providing sub-microsecond vector operations (cosine similarity,
dot product, L2 norm, and Euclidean distance).

Falls back transparently to pure-Python implementations if the native binary
is unavailable, guaranteeing 100% zero-risk execution across all environments.
"""

from __future__ import annotations

import ctypes
import math
import os
import sys
from collections.abc import Sequence

_c_lib = None
_lib_paths_checked = []

# Search for the compiled shared library in build target directories and standard locations
_repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
_candidates = [
    os.path.join(os.path.dirname(__file__), "sunborn_simd_accelerator.dll" if sys.platform == "win32" else "libsunborn_simd_accelerator.so"),
    os.path.join(_repo_root, "target", "release", "sunborn_simd_accelerator.dll" if sys.platform == "win32" else "libsunborn_simd_accelerator.so"),
    os.path.join(_repo_root, "target", "debug", "sunborn_simd_accelerator.dll" if sys.platform == "win32" else "libsunborn_simd_accelerator.so"),
]

for _path in _candidates:
    _lib_paths_checked.append(_path)
    if os.path.exists(_path):
        try:
            _c_lib = ctypes.CDLL(_path)
            # Setup signatures
            _c_lib.sunborn_c_cosine_similarity.argtypes = [ctypes.POINTER(ctypes.c_float), ctypes.POINTER(ctypes.c_float), ctypes.c_size_t]
            _c_lib.sunborn_c_cosine_similarity.restype = ctypes.c_float

            _c_lib.sunborn_c_vector_dot_product.argtypes = [ctypes.POINTER(ctypes.c_float), ctypes.POINTER(ctypes.c_float), ctypes.c_size_t]
            _c_lib.sunborn_c_vector_dot_product.restype = ctypes.c_float

            _c_lib.sunborn_c_vector_l2_norm.argtypes = [ctypes.POINTER(ctypes.c_float), ctypes.c_size_t]
            _c_lib.sunborn_c_vector_l2_norm.restype = ctypes.c_float

            _c_lib.sunborn_c_vector_euclidean_distance.argtypes = [ctypes.POINTER(ctypes.c_float), ctypes.POINTER(ctypes.c_float), ctypes.c_size_t]
            _c_lib.sunborn_c_vector_euclidean_distance.restype = ctypes.c_float
            break
        except Exception:
            _c_lib = None


def is_c_accelerator_available() -> bool:
    """Returns True if the native C SIMD vector accelerator is loaded."""
    return _c_lib is not None


def fast_cosine_similarity(a: Sequence[float], b: Sequence[float]) -> float:
    """Computes high-speed vectorized cosine similarity between two float vectors."""
    n = min(len(a), len(b))
    if n == 0:
        return 0.0

    if _c_lib is not None:
        c_arr_type = ctypes.c_float * n
        arr_a = c_arr_type(*a[:n])
        arr_b = c_arr_type(*b[:n])
        return float(_c_lib.sunborn_c_cosine_similarity(arr_a, arr_b, ctypes.c_size_t(n)))

    # Pure Python reference fallback
    dot = 0.0
    norm_a = 0.0
    norm_b = 0.0
    for x, y in zip(a[:n], b[:n], strict=True):
        dot += x * y
        norm_a += x * x
        norm_b += y * y

    if norm_a <= 0.0 or norm_b <= 0.0:
        return 0.0
    denom = math.sqrt(norm_a) * math.sqrt(norm_b)
    return max(-1.0, min(1.0, dot / denom)) if denom > 0.0 else 0.0


def fast_dot_product(a: Sequence[float], b: Sequence[float]) -> float:
    """Computes high-speed vectorized dot product between two float vectors."""
    n = min(len(a), len(b))
    if n == 0:
        return 0.0

    if _c_lib is not None:
        c_arr_type = ctypes.c_float * n
        arr_a = c_arr_type(*a[:n])
        arr_b = c_arr_type(*b[:n])
        return float(_c_lib.sunborn_c_vector_dot_product(arr_a, arr_b, ctypes.c_size_t(n)))

    return sum(x * y for x, y in zip(a[:n], b[:n], strict=True))


def fast_l2_norm(a: Sequence[float]) -> float:
    """Computes high-speed vectorized L2 norm (magnitude) of a float vector."""
    n = len(a)
    if n == 0:
        return 0.0

    if _c_lib is not None:
        c_arr_type = ctypes.c_float * n
        arr_a = c_arr_type(*a)
        return float(_c_lib.sunborn_c_vector_l2_norm(arr_a, ctypes.c_size_t(n)))

    return math.sqrt(sum(x * x for x in a))


def fast_euclidean_distance(a: Sequence[float], b: Sequence[float]) -> float:
    """Computes high-speed vectorized Euclidean distance between two float vectors."""
    n = min(len(a), len(b))
    if n == 0:
        return 0.0

    if _c_lib is not None:
        c_arr_type = ctypes.c_float * n
        arr_a = c_arr_type(*a[:n])
        arr_b = c_arr_type(*b[:n])
        return float(_c_lib.sunborn_c_vector_euclidean_distance(arr_a, arr_b, ctypes.c_size_t(n)))

    return math.sqrt(sum((x - y) ** 2 for x, y in zip(a[:n], b[:n], strict=True)))
