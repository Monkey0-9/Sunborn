"""
Sunborn Enterprise AI Framework Integrations (Agent & Index Caching).
"""

from .langchain import SunbornLangChainCache
from .llamaindex import SunbornLlamaIndexCache

# Universal framework aliases
SunbornAgentCache = SunbornLangChainCache
SunbornIndexCache = SunbornLlamaIndexCache

__all__ = [
    "SunbornAgentCache",
    "SunbornIndexCache",
    "SunbornLangChainCache",
    "SunbornLlamaIndexCache",
]
